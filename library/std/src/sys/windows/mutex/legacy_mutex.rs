use crate::cell::UnsafeCell;
use crate::io;
use crate::ptr;
use crate::sys::{c, cvt};

/// Mutex based on `CreateMutex`.
///
/// Slow, but available everywhere. Since it is handle-based it's also movable, but not
/// `const`-buildable.
#[repr(transparent)]
pub struct LegacyMutex {
    handle: UnsafeCell<c::HANDLE>,
}

unsafe impl Send for LegacyMutex {}
unsafe impl Sync for LegacyMutex {}

impl LegacyMutex {
    pub const fn new() -> Self {
        Self { handle: UnsafeCell::new(ptr::null_mut()) }
    }

    #[inline]
    pub unsafe fn init(&self) {
        let handle = c::CreateMutexA(ptr::null_mut(), c::FALSE, ptr::null());

        if handle.is_null() {
            panic!("failed creating mutex: {}", io::Error::last_os_error());
        }

        *self.handle.get() = handle;
    }

    #[inline]
    pub unsafe fn lock(&self) {
        if c::WaitForSingleObject(*self.handle.get(), c::INFINITE) != c::WAIT_OBJECT_0 {
            panic!("mutex lock failed: {}", io::Error::last_os_error())
        }
    }

    #[inline]
    pub unsafe fn try_lock(&self) -> bool {
        match c::WaitForSingleObject(*self.handle.get(), 0) {
            c::WAIT_OBJECT_0 => true,
            c::WAIT_TIMEOUT => false,
            _ => panic!("try lock error: {}", io::Error::last_os_error()),
        }
    }

    #[inline]
    pub unsafe fn unlock(&self) {
        cvt(c::ReleaseMutex(*self.handle.get())).unwrap();
    }

    #[inline]
    pub unsafe fn destroy(&self) {
        cvt(c::CloseHandle(*self.handle.get())).unwrap();
    }
}
