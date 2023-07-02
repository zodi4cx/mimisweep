use crate::memory::{self, MemoryHandle};
use crate::process::{self, ImageNtHeaders};
use crate::Board;

use anyhow::{anyhow, bail, Context, Result};
use colored::*;
use lazy_static::lazy_static;
use log::{debug, trace};
use std::ffi::c_void;

const WIN6_SAFE_GET_SINGLETON: [u8; 14] = [
    0x48, 0x89, 0x44, 0x24, 0x70, 0x48, 0x85, 0xc0, 0x74, 0x0a, 0x48, 0x8b, 0xc8, 0xe8,
];
const OFFS_WIN6_TO_G: isize = -21;

lazy_static! {
    static ref DISP_MINESWEEPER: Vec<ColoredString> = vec![
        "0".into(),
        "1".blue(),
        "2".green(),
        "3".red(),
        "4".purple(),
        "5".truecolor(94, 9, 28),
        "6".cyan(),
        "7".bright_blue(),
        "8".bright_green(),
        ".".into(),
        "F".on_red(),
        "?".black().on_white(),
        " ".into(),
        "!".red(),
        "!".red().bold(),
    ];
}

#[repr(C)]
struct MinesweeperElement {
    cb_elements: u32,
    unk0: u32,
    unk1: u32,
    elements: *mut c_void,
    unk2: u32,
    unk3: u32,
}

#[derive(Clone)]
#[repr(C)]
struct PMinesweeperElement(*const MinesweeperElement);

impl Default for PMinesweeperElement {
    fn default() -> Self {
        PMinesweeperElement(std::ptr::null())
    }
}

#[repr(C)]
struct MinesweeperBoard {
    serializer: *mut c_void,
    cb_mines: u32,
    cb_rows: u32,
    cb_columns: u32,
    unk0: u32,
    unk1: u32,
    unk2: u32,
    unk3: u32,
    unk4: u32,
    unk5: u32,
    unk6: u32,
    unk7: u32,
    unk8: u32,
    unk9: u32,
    unk10: *mut c_void,
    unk11: *mut c_void,
    ref_visibles: *mut MinesweeperElement,
    ref_mines: *mut MinesweeperElement,
    unk12: u32,
    unk13: u32,
}

#[repr(C)]
struct MinesweeperGame {
    serializer: *mut c_void,
    p_node_base: *mut c_void,
    p_board_canvas: *mut c_void,
    p_board: *mut MinesweeperBoard,
}

enum Visibility {
    Revealed,
    Hidden,
}

pub fn board(a_remote: MemoryHandle) -> Result<Board> {
    debug!("Accessing Minesweeper's PEB");
    let peb = process::peb(&a_remote, false).context("unable to access process' PEB")?;
    trace!("PEB Image Base address: {:#?}", peb.image_base_address);
    let ntheaders = process::nt_headers(&a_remote, peb.image_base_address)
        .context("unable to access process' NT header")?;
    let (image_base, image_size) = match ntheaders {
        ImageNtHeaders::X64(headers) => (
            headers.OptionalHeader.ImageBase as *const _,
            headers.OptionalHeader.SizeOfImage,
        ),
        ImageNtHeaders::X32(_) => bail!("x86 minesweeper not yet supported"),
    };
    trace!("NT Image Base address:  {:?}", image_base);
    trace!("NT Image size: {:#x}", image_size);
    debug!("Finding game structure in-memory");
    let get_singleton_instruction_offset =
        memory::search(&WIN6_SAFE_GET_SINGLETON, &a_remote, image_base, image_size)?
            .ok_or(anyhow!("Get Singleton pattern not found in-memory"))?;
    trace!(
        "Get Singleton at offset {:#x}",
        get_singleton_instruction_offset
    );
    let board = unsafe {
        let p_g_offset =
            image_base.offset(get_singleton_instruction_offset as isize + OFFS_WIN6_TO_G);
        let g_offset: u32 = memory::copy(&a_remote, p_g_offset as *const _)?;
        // if minesweeper is x64
        let p_g = p_g_offset.offset(1 + std::mem::size_of::<u32>() as isize + g_offset as isize);
        trace!("G address: {:?}", p_g);
        let p_game: *const MinesweeperGame = memory::copy(&a_remote, p_g as *const _)?;
        trace!("Game address: {:?}", p_game);
        let game = memory::copy(&a_remote, p_game)?;
        memory::copy(&a_remote, game.p_board)?
    };
    debug!("Parsing data from game board");
    let mut parsed_board = Board::new(
        board.cb_rows as usize,
        board.cb_columns as usize,
        board.cb_mines,
    );
    unsafe {
        parse_raw_board(
            &a_remote,
            &mut parsed_board,
            board.ref_visibles,
            Visibility::Revealed,
        )
        .context("Unexpected error parsing visible fields")?;
        parse_raw_board(
            &a_remote,
            &mut parsed_board,
            board.ref_mines,
            Visibility::Hidden,
        )
        .context("Unexpected error parsing mine fields")?;
    }
    Ok(parsed_board)
}

unsafe fn parse_raw_board(
    memory: &MemoryHandle,
    board: &mut Board,
    base: *const MinesweeperElement,
    visible: Visibility,
) -> Result<()> {
    let root_element = memory::copy(memory, base).context("failed to retrieve root element")?;
    let columns = root_element.cb_elements as usize;
    let columns_data: Vec<PMinesweeperElement> =
        memory::copy_array(memory, root_element.elements as *const _, columns)
            .context("failed to retrieve column pointers")?;
    for (c, column) in columns_data.iter().enumerate() {
        let column = memory::copy(memory, column.0 as *const MinesweeperElement)
            .context("failed to retrieve column data")?;
        let rows = column.cb_elements as usize;
        match visible {
            Visibility::Revealed => {
                let rows_data = memory::copy_array(memory, column.elements as *const u32, rows)
                    .context(format!("failed to retrieve rows from column {c}"))?;
                for (r, row) in rows_data.iter().enumerate() {
                    board.insert(&DISP_MINESWEEPER[*row as usize], r, c)?;
                }
            }
            Visibility::Hidden => {
                let rows_data = memory::copy_array(memory, column.elements as *const u8, rows)
                    .context(format!("failed to retrieve rows from column {c}"))?;
                for (r, row) in rows_data.iter().enumerate() {
                    if *row != 0 {
                        board.insert(&"*".bright_red(), r, c)?;
                    }
                }
            }
        }
    }
    Ok(())
}
