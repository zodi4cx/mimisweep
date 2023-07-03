//! Implementation of the mimikatz minesweper module, supporting the Windows XP
//! and Windows 7 variants of the game.

#![warn(missing_docs)]

pub mod memory;
pub mod process;
mod versions;

pub use anyhow::Result;

use memory::MemoryHandle;
use versions::{windows_7 as win7, windows_xp as winxp};

use anyhow::{bail, ensure, Context};
use colored::*;
use log::{debug, trace};
use std::{
    collections::HashMap,
    fmt::{self, Display},
};
use windows::Win32::{Foundation::*, System::Threading::*};

/// Abstract representation of a Minesweeper game board, meant to be used
/// for displaying the game state to the user.
#[doc(hidden)]
pub struct Board {
    mines: u32,
    rows: usize,
    columns: usize,
    data: Vec<Vec<ColoredString>>,
}

impl Board {
    fn new(rows: usize, columns: usize, mines: u32) -> Board {
        Board {
            mines,
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
            writeln!(f)?;
        }
        Ok(())
    }
}

enum Version {
    WindowsXP,
    Windows7,
}

impl Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let version = match self {
            Version::Windows7 => "Windows 7",
            Version::WindowsXP => "Windows XP",
        };
        write!(f, "{}", version)
    }
}

/// Command for retrieving information about the state of an active Minesweeper
/// game.
///
/// Running process will be searched for a known game implemenetation. If
/// found, the game is accessed in-memory and the information relevant is retrieved
/// and displayed on screen.
pub fn info() -> Result<()> {
    let version_map = HashMap::from([
        ("Minesweeper.exe", Version::Windows7),
        ("WINMINE.EXE", Version::WindowsXP),
    ]);
    debug!("Opening Minesweeper process");
    let Some((pid, version)) = version_map.iter().find_map(|(name, version)| {
        process::pid_by_name(name).map(|pid| (pid, version))
    }) else {
        bail!("no minesweeper in memory!");
    };
    debug!("Detected {} version running", version);
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
    let board = match version {
        Version::WindowsXP => winxp::board(a_remote),
        Version::Windows7 => win7::board(a_remote),
    }
    .context("unable to retrieve game board")?;
    println!(
        "Field: {} r x {} c, Mines: {}",
        board.rows, board.columns, board.mines
    );
    println!("\n{board}");
    Ok(())
}
