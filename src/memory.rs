//! Memory-releated tools used to interface with Windows processes.

use anyhow::{anyhow, Context, Result};
use memchr::memmem;
use std::{ffi::c_void, mem, ops::Deref, ptr::addr_of_mut};
use windows::Win32::{Foundation::*, System::Diagnostics::Debug::*};

/// Memory handle abstraction for dealing with different types of memory access.
/// Implements the RAII pattern for automatic deallocation of any associated handles.
#[allow(dead_code)]
#[derive(Debug)]
pub enum MemoryHandle {
    /// Own process memory
    Own,
    /// Running process memory
    Process(HANDLE),
    /// File access
    File(HANDLE),
    /// Kernel access
    Kernel(HANDLE),
    /// Memory dump
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

/// Returns a copy of an object, read from the resource pointed by the given
/// [`MemoryHandle`].
///
/// # Safety
///
/// The `data_ptr` argument is expected to point to a valid resource of the
/// specified type. The caller is responsbile for checking if the returned object
/// is indeed a valid instance of the requested type.
pub unsafe fn copy<T>(memory: &MemoryHandle, data_ptr: *const T) -> Result<T> {
    match memory {
        MemoryHandle::Process(handle) => read_from_process(*handle, data_ptr),
        _ => unimplemented!("copy not implemented for {:?}", memory),
    }
}

unsafe fn read_from_process<T>(process: HANDLE, data_ptr: *const T) -> Result<T> {
    let mut data: T = mem::zeroed();
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

/// Returns a vector of elements, read from the resource pointed by the given
/// [`MemoryHandle`].
///
/// # Safety
///
/// The `data_ptr` argument is expected to point to a valid resource of the
/// specified type. The caller is responsbile for checking if the returned vector
/// holds copies of valid instances of the requested type.
pub unsafe fn copy_array<T>(
    memory: &MemoryHandle,
    data_ptr: *const T,
    count: usize,
) -> Result<Vec<T>>
where
    T: Clone + Default,
{
    match memory {
        MemoryHandle::Process(handle) => read_array_from_process(*handle, data_ptr, count),
        _ => unimplemented!("copy_array not implemented for {:?}", memory),
    }
}

unsafe fn read_array_from_process<T>(
    process: HANDLE,
    data_ptr: *const T,
    count: usize,
) -> Result<Vec<T>>
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

/// Searches a pattern of bytes in-memory, starting from the `base` address up
/// to `size` bytes, returning the first coincidence. If the pattern is found,
/// the index of the starting byte of the sequence is returned.
pub fn search(
    pattern: &[u8],
    memory: &MemoryHandle,
    base: *const c_void,
    size: u32,
) -> Result<Option<usize>> {
    match memory {
        MemoryHandle::Process(_) | MemoryHandle::File(_) | MemoryHandle::Kernel(_) => {
            let data: Vec<u8> = unsafe { copy_array(memory, base as *const _, size as usize) }
                .context("failed to copy haystack")?;
            Ok(memmem::find(&data, pattern))
        }
        _ => unimplemented!("search not implemented for {:?}", memory),
    }
}
