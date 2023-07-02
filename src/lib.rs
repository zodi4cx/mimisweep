mod memory;
mod process;
mod utils;
mod versions;

use memory::MemoryHandle;
use versions::{windows_7 as win7, windows_xp as winxp};

use anyhow::{bail, ensure, Context, Result};
use colored::*;
use log::{debug, trace};
use std::{
    collections::HashMap,
    fmt::{self, Display},
};
use windows::Win32::{Foundation::*, System::Threading::*};

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

pub fn info() -> Result<()> {
    let version_map = HashMap::from([
        ("Minesweeper.exe", Version::Windows7),
        ("WINMINE.EXE", Version::WindowsXP),
    ]);
    debug!("Opening Minesweeper process");
    let Some((pid, version)) = version_map.iter().find_map(|(name, version)| {
        utils::process_pid_by_name(name).map(|pid| (pid, version))
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
