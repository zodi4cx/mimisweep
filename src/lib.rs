mod memory;
mod process;
mod utils;

use memory::MemoryHandle;

use anyhow::{bail, Context, Result};
use log::{debug, trace};
#[allow(unused_imports)]
use windows::Win32::{
    Foundation::*,
    System::{Diagnostics::Debug::*, Kernel::*, Threading::*},
};

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
    let ntheaders: IMAGE_NT_HEADERS64 = process::nt_headers(&a_remote, peb.image_base_address)
        .context("unable to access process' NT header")?;
    let image_base = ntheaders.OptionalHeader.ImageBase;
    trace!("NT Image Base address:  {:#018x}", image_base);
    Ok(())
}
