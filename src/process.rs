use super::memory::MemoryHandle;

use anyhow::{anyhow, ensure, Result};
use log::trace;
use std::{mem, ptr::addr_of_mut, ffi::c_void};
#[allow(unused_imports)]
use windows::Win32::{
    Foundation::*,
    System::{Diagnostics::Debug::*, Kernel::*, Threading::*},
};

/// PEB definition that overrides windows' PEB struct based on online documentation.
#[repr(C)]
pub struct Peb {
    pub inherited_address_space: u8,
    pub read_image_file_exec_options: u8,
    pub being_debugged: u8,
    pub bit_field: BitField,
    pub mutant: HANDLE,
    pub image_base_address: *mut c_void,
    pub ldr: *mut PEB_LDR_DATA,
    pub process_parameters: *mut RTL_USER_PROCESS_PARAMETERS,
    // ...
}

#[repr(C)]
pub struct BitField {
    pub image_uses_large_pages: u8,
    pub spare_bits: u8,
}

/// Retrieves the PEB structure of the given memory handle
pub fn peb(memory: &MemoryHandle, _is_wow: bool) -> Result<Peb> {
    match memory {
        MemoryHandle::Process(handle) => peb_process(handle, _is_wow),
        _ => unimplemented!("PEB extraction for {:?} is not implemented", memory)
    }
}

fn peb_process(process: &HANDLE, _is_wow: bool) -> Result<Peb> {
    unsafe {
        let mut return_length = 0_u32;
        let mut process_informations: PROCESS_BASIC_INFORMATION = mem::zeroed();
        let process_information_length = mem::size_of::<PROCESS_BASIC_INFORMATION>() as u32;
        trace!("About to call NtQueryInformationProcess");
        NtQueryInformationProcess(
            *process,
            ProcessBasicInformation,
            &mut process_informations as *mut _ as _,
            process_information_length,
            &mut return_length as *mut u32,
        )?;
        ensure!(
            process_information_length == return_length,
            "unexpected result from NtQueryInformationProcess"
        );
        trace!("PEB address: {:?}", process_informations.PebBaseAddress);
        read_from_process(process, process_informations.PebBaseAddress as *mut Peb)
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
    .ok_or(anyhow!("error reading memory of remote process"))
}