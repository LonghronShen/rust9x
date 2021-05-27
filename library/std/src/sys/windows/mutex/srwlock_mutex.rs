use crate::cell::UnsafeCell;
use crate::sys::c;

pub struct SrwLockMutex {
    srwlock: UnsafeCell<c::SRWLOCK>,
}

unsafe impl Send for SrwLockMutex {}
unsafe impl Sync for SrwLockMutex {}

impl SrwLockMutex {
    #[inline]
    pub fn raw(&self) -> c::PSRWLOCK {
        self.srwlock.get()
    }

    pub const fn new() -> Self {
        Self { srwlock: UnsafeCell::new(c::SRWLOCK_INIT) }
    }

    #[inline]
    pub unsafe fn init(&mut self) {}

    #[inline]
    pub unsafe fn lock(&self) {
        c::AcquireSRWLockExclusive(self.raw());
    }

    #[inline]
    pub unsafe fn try_lock(&self) -> bool {
        c::TryAcquireSRWLockExclusive(self.raw()) != 0
    }

    #[inline]
    pub unsafe fn unlock(&self) {
        c::ReleaseSRWLockExclusive(self.raw());
    }

    #[inline]
    pub unsafe fn destroy(&self) {
        // SRWLock does not need to be destroyed.
    }
}
