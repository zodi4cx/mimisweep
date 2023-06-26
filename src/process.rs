use super::memory::MemoryHandle;

use anyhow::{anyhow, ensure, Result};
use log::trace;
use std::{mem, ptr::addr_of_mut};
#[allow(unused_imports)]
use windows::Win32::{
    Foundation::*,
    System::{Diagnostics::Debug::*, Kernel::*, Threading::*},
};

/// Retrieves the PEB structure of the given memory handle
pub fn peb(memory: &MemoryHandle, _is_wow: bool) -> Result<PEB> {
    match memory {
        MemoryHandle::Process(handle) => peb_process(handle, _is_wow),
        _ => unimplemented!("PEB extraction for {:?} is not implemented", memory)
    }
}

fn peb_process(memory: &HANDLE, _is_wow: bool) -> Result<PEB> {
    unsafe {
        let mut return_length = 0_u32;
        let mut process_informations: PROCESS_BASIC_INFORMATION = mem::zeroed();
        let process_information_length = mem::size_of::<PROCESS_BASIC_INFORMATION>() as u32;
        trace!("About to call NtQueryInformationProcess");
        NtQueryInformationProcess(
            *memory,
            ProcessBasicInformation,
            &mut process_informations as *mut _ as _,
            process_information_length,
            &mut return_length as *mut u32,
        )?;
        ensure!(
            process_information_length == return_length,
            "unexpected result from NtQueryInformationProcess"
        );
        trace!("Finished call to NtQueryInformationProcess");
        read_from_process(memory, process_informations.PebBaseAddress)
    }
}

fn read_from_process<T>(process: &HANDLE, data_ptr: *mut T) -> Result<T> {
    let mut data: T = unsafe { mem::zeroed() };
    unsafe {
        ReadProcessMemory(
            *process,
            data_ptr as *mut _,
            addr_of_mut!(data) as *mut _,
            mem::size_of::<T>(),
            None,
        )
    }
    .as_bool()
    .then_some(data)
    .ok_or(anyhow!("Error reading memory of remote process"))
}
