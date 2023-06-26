mod memory;
mod process;
mod utils;

use memory::MemoryHandle;

use anyhow::{bail, Context, Result};
use log::{debug, trace};
#[allow(unused_imports)]
use windows::{
    Win32::{
        Foundation::*,
        System::{Kernel::*, Threading::*},
    },
};

pub fn info() -> Result<()> {
    debug!("Opening Minesweeper process");
    let Some(pid) = utils::process_pid_by_name("minesweeper.exe") else {
        bail!("No minesweeper in memory!");
    };
    trace!("Minesweeper PID: {pid}");
    let a_remote = unsafe {
        let h_process: HANDLE = OpenProcess(
            PROCESS_VM_READ | PROCESS_VM_OPERATION | PROCESS_QUERY_INFORMATION,
            false,
            pid,
        )
        .context("Failed to open process")?;
        trace!("Process handle: {:?}", h_process);
        MemoryHandle::Process(h_process)
    };
    debug!("Extracting PE headers");
    let peb: PEB = process::peb(&a_remote, false).context("Unable to access process' PEB")?;
    trace!("Retrieved PEB base address");
    debug!("{:#?}", peb.SessionId);
    Ok(())
}
