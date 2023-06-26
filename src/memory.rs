use windows::Win32::Foundation::*;
use std::ops::Deref;

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