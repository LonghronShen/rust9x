use crate::sync::atomic::{AtomicUsize, Ordering};
use crate::sys::c;
use crate::sys::mutex;
use crate::sys::mutex::compat::{atomic_init, MutexKind, MUTEX_KIND};

pub struct RWLock {
    lock: AtomicUsize,
}

unsafe impl Send for RWLock {}
unsafe impl Sync for RWLock {}

impl RWLock {
    pub const fn new() -> RWLock {
        // SRWLock is usize-sized
        RWLock { lock: AtomicUsize::new(0) }
    }
    #[inline]
    pub unsafe fn read(&self) {
        match MUTEX_KIND {
            MutexKind::SrwLock => c::AcquireSRWLockShared(&self.lock as *const _ as *mut _),
            MutexKind::CriticalSection | MutexKind::Legacy => (*self.remutex()).lock(),
        }
    }
    #[inline]
    pub unsafe fn try_read(&self) -> bool {
        match MUTEX_KIND {
            MutexKind::SrwLock => c::TryAcquireSRWLockShared(&self.lock as *const _ as *mut _) != 0,
            MutexKind::CriticalSection | MutexKind::Legacy => (*self.remutex()).try_lock(),
        }
    }
    #[inline]
    pub unsafe fn write(&self) {
        match MUTEX_KIND {
            MutexKind::SrwLock => c::AcquireSRWLockExclusive(&self.lock as *const _ as *mut _),
            MutexKind::CriticalSection | MutexKind::Legacy => (*self.remutex()).lock(),
        }
    }
    #[inline]
    pub unsafe fn try_write(&self) -> bool {
        match MUTEX_KIND {
            MutexKind::SrwLock => {
                c::TryAcquireSRWLockExclusive(&self.lock as *const _ as *mut _) != 0
            }
            MutexKind::CriticalSection | MutexKind::Legacy => (*self.remutex()).try_lock(),
        }
    }
    #[inline]
    pub unsafe fn read_unlock(&self) {
        match MUTEX_KIND {
            MutexKind::SrwLock => c::ReleaseSRWLockShared(&self.lock as *const _ as *mut _),
            MutexKind::CriticalSection | MutexKind::Legacy => (*self.remutex()).unlock(),
        }
    }
    #[inline]
    pub unsafe fn write_unlock(&self) {
        match MUTEX_KIND {
            MutexKind::SrwLock => c::ReleaseSRWLockExclusive(&self.lock as *const _ as *mut _),
            MutexKind::CriticalSection | MutexKind::Legacy => (*self.remutex()).unlock(),
        }
    }

    #[inline]
    pub unsafe fn destroy(&self) {
        match MUTEX_KIND {
            MutexKind::SrwLock => {}
            MutexKind::CriticalSection | MutexKind::Legacy => {
                match self.lock.load(Ordering::SeqCst) {
                    0 => {}
                    n => {
                        Box::from_raw(n as *mut mutex::Mutex).destroy();
                    }
                }
            }
        }
    }

    unsafe fn remutex(&self) -> *mut mutex::Mutex {
        unsafe fn init() -> Box<mutex::Mutex> {
            let mut re = box mutex::Mutex::new();
            re.init();
            re
        }

        unsafe fn destroy(mutex: &mutex::Mutex) {
            mutex.destroy()
        }

        atomic_init(&self.lock, init, destroy)
    }
}
