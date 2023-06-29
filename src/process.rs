use super::memory::{self, MemoryHandle};

use anyhow::{anyhow, ensure, Result};
use log::trace;
use std::{ffi::c_void, mem};
use windows::Win32::System::SystemInformation::IMAGE_FILE_MACHINE_I386;
#[allow(unused_imports)]
use windows::Win32::{
    Foundation::*,
    System::{Diagnostics::Debug::*, Kernel::*, SystemServices::*, Threading::*},
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
        MemoryHandle::Process(_) => peb_process(memory, _is_wow),
        _ => unimplemented!("PEB extraction for {:?} is not implemented", memory),
    }
}

fn peb_process(memory: &MemoryHandle, _is_wow: bool) -> Result<Peb> {
    unsafe {
        let mut return_length = 0_u32;
        let mut process_informations: PROCESS_BASIC_INFORMATION = mem::zeroed();
        let process_information_length = mem::size_of::<PROCESS_BASIC_INFORMATION>() as u32;
        trace!("About to call NtQueryInformationProcess");
        NtQueryInformationProcess(
            **memory,
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
        memory::copy(memory, process_informations.PebBaseAddress as *const Peb)
    }
}

#[repr(C)]
struct ImageNtHeadersCommon {
    signature: u32,
    file_header: IMAGE_FILE_HEADER,
    // optional header omitted (architecture dependant)
}

pub enum ImageNtHeaders {
    X32(IMAGE_NT_HEADERS32),
    X64(IMAGE_NT_HEADERS64),
}

impl ImageNtHeaders {
    fn is_valid(&self) -> bool {
        match self {
            Self::X32(header) => header.Signature == IMAGE_NT_SIGNATURE,
            Self::X64(header) => header.Signature == IMAGE_NT_SIGNATURE,
        }
    }
}

pub fn nt_headers(process: &MemoryHandle, base: *const c_void) -> Result<ImageNtHeaders> {
    let header_image_dos: IMAGE_DOS_HEADER = unsafe { memory::copy(process, base as *const _)? };
    ensure!(
        header_image_dos.e_magic == IMAGE_DOS_SIGNATURE,
        "invalid DOS signature"
    );
    let nt_headers = unsafe {
        let p_nt_headers = base.offset(header_image_dos.e_lfanew as isize);
        let nt_common: ImageNtHeadersCommon = memory::copy(process, p_nt_headers as *const _)?;
        match nt_common.file_header.Machine {
            IMAGE_FILE_MACHINE_I386 => {
                let headers_32: IMAGE_NT_HEADERS32 = memory::copy(process, p_nt_headers as *const _)?;
                ImageNtHeaders::X32(headers_32)
            }
            _ => {
                let headers_64: IMAGE_NT_HEADERS64 = memory::copy(process, p_nt_headers as *const _)?;
                ImageNtHeaders::X64(headers_64)
            }
        }
    };
    nt_headers
        .is_valid()
        .then_some(nt_headers)
        .ok_or(anyhow!("invalid NT signature"))
}
