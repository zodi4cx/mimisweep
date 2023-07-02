//! This modules contains the necessary functions to interface with the
//! Windows XP version of Minesweeper.

use crate::memory::{self, MemoryHandle};
use crate::Board;

use anyhow::{ensure, Result};
use colored::*;
use lazy_static::lazy_static;
use log::{debug, trace};

const WINXP_BOARD_ADDRESS: u32 = 0x01005330;
const WINXP_BOARD_SIZE: usize = 0x360;
const FIELD_SIZE: usize = 0x20;
const CELL_DELIMITER: u8 = 0x10;
const CELL_EMPTY: u8 = 0x0f;

lazy_static! {
    static ref DISP_MINESWEEPER: Vec<ColoredString> = vec![
        " ".into(),
        "1".blue(),
        "2".green(),
        "3".red(),
        "4".purple(),
        "5".truecolor(94, 9, 28),
        "6".cyan(),
        "7".bright_blue(),
        "8".bright_green(),
        ".".into(),
        "*".bright_red(),
        "F".on_red(),
        "?".black().on_white(),
        "!".red().bold(),
    ];
}

enum Element {
    Hidden = 9,
    Mine = 10,
    Flag = 11,
    Mark = 12,
    Unknown = 13,
}

#[repr(C)]
struct MinesweeperBoard {
    mines: u32,
    width: u32,
    height: u32,
    unk0: u32,
    data: [u8; WINXP_BOARD_SIZE],
}

/// Retrieve the board state from the provided process.
pub fn board(a_remote: MemoryHandle) -> Result<Board> {
    debug!("Reading game board state");
    let board = unsafe {
        let p_board = WINXP_BOARD_ADDRESS as *const _;
        let board: MinesweeperBoard = memory::copy(&a_remote, p_board)?;
        ensure!(9 <= board.width && board.width <= 30, "invalid board width");
        ensure!(
            9 <= board.height && board.height <= 24,
            "invalid board height"
        );
        trace!("Board: {} c x {} r", board.width, board.height);
        let (header, empty) = board.data[..FIELD_SIZE].split_at((board.width + 2) as _);
        ensure!(
            header.iter().all(|&n| n == CELL_DELIMITER) && empty.iter().all(|&n| n == CELL_EMPTY),
            "invalid board structure",
        );
        board
    };
    let mut parsed_board = Board::new(board.height as _, board.width as _, board.mines);

    for (r, data) in board
        .data
        .chunks(FIELD_SIZE)
        .skip(1)
        .take(parsed_board.rows)
        .map(|row| &row[1..=board.width as _])
        .enumerate()
    {
        for (c, cell) in data.iter().enumerate() {
            let value = match cell {
                _ if cell & 0x80 != 0 || *cell == 0xcc => &DISP_MINESWEEPER[Element::Mine as usize],
                _ if cell & 0x0f == 0x0e => &DISP_MINESWEEPER[Element::Flag as usize],
                _ if cell & 0x0f == 0x0d => &DISP_MINESWEEPER[Element::Mark as usize],
                _ if cell & 0xf0 == 0 => &DISP_MINESWEEPER[Element::Hidden as usize],
                _ if cell & 0x40 != 0 => &DISP_MINESWEEPER[(cell & 0x0f) as usize],
                _ => &DISP_MINESWEEPER[Element::Unknown as usize],
            };
            parsed_board.insert(value, r, c).unwrap();
        }
    }
    Ok(parsed_board)
}
