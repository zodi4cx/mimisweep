//! Tools for interaction with Windows processes.

use super::memory::{self, MemoryHandle};

use anyhow::{anyhow, ensure, Result};
use log::trace;
use std::{ffi::c_void, mem};
use sysinfo::{PidExt, ProcessExt, System, SystemExt};
use windows::Win32::System::SystemInformation::IMAGE_FILE_MACHINE_I386;
#[allow(unused_imports)]
use windows::Win32::{
    Foundation::*,
    System::{Diagnostics::Debug::*, Kernel::*, SystemServices::*, Threading::*},
};

/// PEB definition that overrides windows' [`PEB`] struct based on WinDbg's symbols.
#[repr(C)]
#[allow(missing_docs)]
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

/// BitField field of the [`Peb`] structure.
#[repr(C)]
#[allow(missing_docs)]
pub struct BitField {
    pub image_uses_large_pages: u8,
    pub spare_bits: u8,
}

/// Given an **exact** process name, it returns its PID, if available.
pub fn pid_by_name(process_name: &str) -> Option<u32> {
    let system = System::new_all();
    let mut processes = system.processes_by_exact_name(process_name);
    (*processes).next().map(|process| process.pid().as_u32())
}

/// Retrieves the [`Peb`] from the given memory handle.
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

/// Wrapper for the NT Headers struct of different architectures.
pub enum ImageNtHeaders {
    /// NT headers of a x86 PE file.
    X86(IMAGE_NT_HEADERS32),
    /// NT headers of a x64 PE file.
    X64(IMAGE_NT_HEADERS64),
}

impl ImageNtHeaders {
    fn is_valid(&self) -> bool {
        match self {
            Self::X86(header) => header.Signature == IMAGE_NT_SIGNATURE,
            Self::X64(header) => header.Signature == IMAGE_NT_SIGNATURE,
        }
    }
}

/// Retrieves the NT Header for a given process.
///
/// # Safety
///
/// The provided image base pointer must be valid. The integrity of the header
/// is checked through the [`IMAGE_DOS_HEADER`] and the [`IMAGE_NT_HEADERS32`]
/// / [`IMAGE_NT_HEADERS64`] signatures before returning the result.
pub unsafe fn nt_headers(
    process: &MemoryHandle,
    image_base: *const c_void,
) -> Result<ImageNtHeaders> {
    ensure!(
        matches!(process, MemoryHandle::Process(_)),
        "a process handle must be provided"
    );
    let dos_header: IMAGE_DOS_HEADER = unsafe { memory::copy(process, image_base as *const _)? };
    ensure!(
        dos_header.e_magic == IMAGE_DOS_SIGNATURE,
        "invalid DOS signature"
    );
    let nt_headers = {
        let p_nt_headers = image_base.offset(dos_header.e_lfanew as isize);
        let nt_common: ImageNtHeadersCommon = memory::copy(process, p_nt_headers as *const _)?;
        match nt_common.file_header.Machine {
            IMAGE_FILE_MACHINE_I386 => {
                let headers_32: IMAGE_NT_HEADERS32 =
                    memory::copy(process, p_nt_headers as *const _)?;
                ImageNtHeaders::X86(headers_32)
            }
            _ => {
                let headers_64: IMAGE_NT_HEADERS64 =
                    memory::copy(process, p_nt_headers as *const _)?;
                ImageNtHeaders::X64(headers_64)
            }
        }
    };
    nt_headers
        .is_valid()
        .then_some(nt_headers)
        .ok_or(anyhow!("invalid NT signature"))
}
