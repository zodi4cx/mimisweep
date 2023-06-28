mod memory;
mod process;
mod utils;

use memory::MemoryHandle;
use process::ImageNtHeaders;

use anyhow::{anyhow, bail, Context, Result};
use log::{debug, trace};
#[allow(unused_imports)]
use windows::Win32::{
    Foundation::*,
    System::{Diagnostics::Debug::*, Kernel::*, Threading::*},
};

const WIN6_SAFE_GET_SINGLETON: [u8; 14] = [
    0x48, 0x89, 0x44, 0x24, 0x70, 0x48, 0x85, 0xc0, 0x74, 0x0a, 0x48, 0x8b, 0xc8, 0xe8,
];

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
            headers.OptionalHeader.ImageBase,
            headers.OptionalHeader.SizeOfImage,
        ),
        ImageNtHeaders::X32(_) => bail!("x86 minesweeper not yet supported"),
    };
    trace!("NT Image Base address:  {:#018x}", image_base);
    trace!("NT Image size: {:#x}", image_size);
    debug!("Finding game structure in-memory");
    let get_singleton_offset = memory::search(
        &WIN6_SAFE_GET_SINGLETON,
        &a_remote,
        image_base as *const _,
        image_size,
    )?
    .ok_or(anyhow!("Get Singleton pattern not found in-memory"))?;
    trace!("Get Singleton at offset {:#x}", get_singleton_offset);
    Ok(())
}
