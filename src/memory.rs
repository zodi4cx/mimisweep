use anyhow::{anyhow, Result, Context};
use memchr::memmem;
use std::{ffi::c_void, mem, ops::Deref, ptr::addr_of_mut};
use windows::Win32::{Foundation::*, System::Diagnostics::Debug::*};

/// Memory handle abstraction for dealing with different types of memory access.
#[allow(dead_code)]
#[derive(Debug)]
pub enum MemoryHandle {
    Own,
    Process(HANDLE),
    File(HANDLE),
    Kernel(HANDLE),
    Dump,
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
        MemoryHandle::Process(handle) => read_from_process(*handle, data_ptr),
        _ => unimplemented!("copy not implemented for {:?}", memory),
    }
}

fn read_from_process<T>(process: HANDLE, data_ptr: *const T) -> Result<T> {
    let mut data: T = unsafe { mem::zeroed() };
    unsafe {
        ReadProcessMemory(
            process,
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

// TODO: refactor this into a single function?
pub fn copy_array<T>(memory: &MemoryHandle, data_ptr: *const T, count: usize) -> Result<Vec<T>>
where
    T: Clone + Default,
{
    match memory {
        MemoryHandle::Process(handle) => read_array_from_process(*handle, data_ptr, count),
        _ => unimplemented!("copy_array not implemented for {:?}", memory),
    }
}

fn read_array_from_process<T>(process: HANDLE, data_ptr: *const T, count: usize) -> Result<Vec<T>>
where
    T: Clone + Default,
{
    let mut vec = vec![Default::default(); count];
    let size = mem::size_of::<T>()
        .checked_mul(count)
        .ok_or(anyhow!("invalid read, overflow in array size"))?;
    unsafe {
        ReadProcessMemory(
            process,
            data_ptr as *mut _,
            vec.as_mut_ptr() as *mut _,
            size,
            None,
        )
    }
    .as_bool()
    .then_some(vec)
    .ok_or(anyhow!("error reading memory of remote process"))
}

pub fn search(
    pattern: &[u8],
    memory: &MemoryHandle,
    base: *const c_void,
    size: u32,
) -> Result<Option<usize>> {
    match memory {
        MemoryHandle::Process(_) | MemoryHandle::File(_) | MemoryHandle::Kernel(_) => {
            let data: Vec<u8> = copy_array(memory, base as *const _, size as usize)
                .context("failed to copy haystack")?;
            Ok(memmem::find(&data, pattern))
        }
        _ => unimplemented!("search not implemented for {:?}", memory),
    }
}
