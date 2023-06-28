mod memory;
mod process;
mod structs;
mod utils;

use memory::MemoryHandle;
use process::ImageNtHeaders;

use anyhow::{anyhow, bail, ensure, Context, Result};
use colored::*;
use lazy_static::lazy_static;
use log::{debug, trace};
use std::{
    ffi::c_void,
    fmt::{self, Display},
};
#[allow(unused_imports)]
use windows::Win32::{
    Foundation::*,
    System::{Diagnostics::Debug::*, Kernel::*, Threading::*},
};

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
        "?".on_white(),
        " ".into(),
        "!".red(),
        "!".red().bold(),
    ];
}

#[repr(C)]
#[derive(Clone)]
struct MinesweeperElement {
    cb_elements: u32,
    unk0: u32,
    unk1: u32,
    elements: *mut c_void,
    unk2: u32,
    unk3: u32,
}

impl Default for MinesweeperElement {
    fn default() -> Self {
        MinesweeperElement {
            cb_elements: 0,
            unk0: 0,
            unk1: 0,
            elements: std::ptr::null_mut(),
            unk2: 0,
            unk3: 0,
        }
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

struct Board {
    rows: usize,
    columns: usize,
    data: Vec<Vec<ColoredString>>,
}

impl Board {
    fn new(rows: usize, columns: usize) -> Board {
        Board {
            rows,
            columns,
            data: vec![vec![" ".into(); columns]; rows],
        }
    }

    fn insert(&mut self, value: &ColoredString, row: usize, column: usize) -> Result<()> {
        ensure!(row < self.rows, "Row {} does not exist", row);
        ensure!(column < self.columns, "Column {} does not exist", column);
        self.data[row][column] = value.clone();
        Ok(())
    }
}

impl Display for Board {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> fmt::Result {
        for r in 0..self.rows {
            write!(f, "\t")?;
            for c in 0..self.columns {
                write!(f, "{} ", self.data[r][c])?;
            }
            write!(f, "\n")?;
        }
        Ok(())
    }
}

enum Visibility {
    Revealed,
    Hidden,
}

pub fn info() -> Result<()> {
    debug!("Opening Minesweeper process");
    let Some(pid) = utils::process_pid_by_name("Minesweeper.exe") else {
        bail!("no minesweeper in memory!");
    };
    trace!("Minesweeper PID: {pid}");
    let a_remote = unsafe {
        let h_process: HANDLE = OpenProcess(
            PROCESS_VM_READ | PROCESS_VM_OPERATION | PROCESS_QUERY_INFORMATION,
            false,
            pid,
        )
        .context("failed to open process")?;
        trace!("Process handle: {:?}", h_process);
        MemoryHandle::Process(h_process)
    };
    debug!("Extracting PE headers");
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
    let pp_get_singleton =
        unsafe { image_base.offset(get_singleton_instruction_offset as isize + OFFS_WIN6_TO_G) };
    let get_singleton_offset: u32 = memory::copy(&a_remote, pp_get_singleton as *const _)?;
    // if minesweeper is x64
    let p_get_singleton = unsafe {
        pp_get_singleton
            .offset(1 + std::mem::size_of::<u32>() as isize + get_singleton_offset as isize)
    };
    trace!("G address: {:?}", p_get_singleton);
    let p_game: *const MinesweeperGame = memory::copy(&a_remote, p_get_singleton as *const _)?;
    trace!("Game address: {:?}", p_game);
    let game = memory::copy(&a_remote, p_game)?;
    let board = memory::copy(&a_remote, game.p_board)?;
    println!(
        "Field: {} r x {} c, Mines: {}",
        board.cb_rows, board.cb_columns, board.cb_mines
    );
    debug!("Parsing data from game board");
    let mut parsed_board = Board::new(board.cb_rows as usize, board.cb_columns as usize);
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
    println!("\n{parsed_board}");
    Ok(())
}

fn parse_raw_board(
    memory: &MemoryHandle,
    board: &mut Board,
    base: *const MinesweeperElement,
    visible: Visibility,
) -> Result<()> {
    let root_element = memory::copy(memory, base).context("failed to retrieve root element")?;
    let columns = root_element.cb_elements as usize;
    let columns_data = memory::copy_array(
        memory,
        root_element.elements as *const MinesweeperElement,
        columns,
    )
    .context("failed to retrieve columns")?;
    for (c, column) in columns_data.iter().enumerate() {
        let rows = column.cb_elements as usize;
        let rows_data =
            memory::copy_array(memory, column.elements as *const MinesweeperElement, rows)
                .context(format!("failed to retrieve rows from column {c}"))?;
        for (r, row) in rows_data.iter().enumerate() {
            match visible {
                Visibility::Revealed => {
                    let index = memory::copy(memory, row.elements as *const u32)
                        .context(format!("failed to retrieve data from {r} r x {c} c"))?;
                    board.insert(&DISP_MINESWEEPER[index as usize], r, c)?;
                }
                Visibility::Hidden => {
                    let index = memory::copy(memory, row.elements as *const u8)
                        .context(format!("failed to retrieve data from {r} r x {c} c"))?;
                    if index != 0 {
                        board.insert(&"*".on_red(), r, c)?;
                    }
                }
            }
        }
    }
    Ok(())
}
