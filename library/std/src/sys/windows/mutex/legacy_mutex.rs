use crate::intrinsics::unlikely;
use crate::ptr;
use crate::sys::mutex::{raw, Mutex};
use crate::sys::{c, cvt};

#[inline]
unsafe fn as_raw(mutex: &Mutex) -> *mut c::HANDLE {
    raw(mutex)
}

#[inline]
pub unsafe fn init(mutex: &Mutex) {
    let handle = as_raw(mutex);

    let new_handle = c::CreateMutexA(ptr::null_mut(), c::FALSE, ptr::null());

    if unlikely(new_handle.is_null()) {
        panic!("failed creating mutex: {}", crate::io::Error::last_os_error());
    }

    *handle = new_handle;
}

#[inline]
pub unsafe fn lock(mutex: &Mutex) {
    let handle = as_raw(mutex);

    if unlikely(c::WaitForSingleObject(*handle, c::INFINITE) != c::WAIT_OBJECT_0) {
        panic!("raw lock failed: {}", crate::io::Error::last_os_error())
    }
}

#[inline]
pub unsafe fn try_lock(mutex: &Mutex) -> bool {
    let handle = as_raw(mutex);

    match c::WaitForSingleObject(*handle, 0) {
        c::WAIT_OBJECT_0 => true,
        c::WAIT_TIMEOUT => false,
        _ => panic!("try lock error: {}", crate::io::Error::last_os_error()),
    }
}

#[inline]
pub unsafe fn unlock(mutex: &Mutex) {
    let handle = as_raw(mutex);

    cvt(c::ReleaseMutex(*handle)).unwrap();
}

#[inline]
pub unsafe fn destroy(mutex: &Mutex) {
    let handle = as_raw(mutex);

    cvt(c::CloseHandle(*handle)).unwrap();
}
