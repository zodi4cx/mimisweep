use anyhow::{anyhow, Result};
use std::mem;
use std::ops::Deref;
use std::ptr::addr_of_mut;
use windows::Win32::{Foundation::*, System::Diagnostics::Debug::*};

/// Memory handle abstraction for dealing with different types of memory access.
#[derive(Debug)]
pub enum MemoryHandle {
    Process(HANDLE),
    _File(HANDLE),
    _Kernel(HANDLE),
}

impl Drop for MemoryHandle {
    fn drop(&mut self) {
        match self {
            Self::Process(handle) => unsafe {
                CloseHandle(*handle);
            },
            _ => unimplemented!("Drop trait not implemented for {:?}", &self),
        }
    }
}

impl Deref for MemoryHandle {
    type Target = HANDLE;

    fn deref(&self) -> &Self::Target {
        match self {
            MemoryHandle::Process(handle) => handle,
            _ => unimplemented!("Deref trait not implemented for {:?}", &self),
        }
    }
}

pub fn copy<T>(memory: &MemoryHandle, data_ptr: *const T) -> Result<T> {
    match memory {
        MemoryHandle::Process(_) => read_from_process(memory, data_ptr),
        _ => unimplemented!("copy not implemented for {:?}", memory),
    }
}

fn read_from_process<T>(process: &MemoryHandle, data_ptr: *const T) -> Result<T> {
    let mut data: T = unsafe { mem::zeroed() };
    unsafe {
        ReadProcessMemory(
            **process,
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
