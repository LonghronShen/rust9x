//! System Mutexes
//!
//! The Windows implementation of mutexes is a little odd and it may not be
//! immediately obvious what's going on. The primary oddness is that SRWLock is
//! used instead of CriticalSection, and this is done because:
//!
//! 1. SRWLock is several times faster than CriticalSection according to
//!    benchmarks performed on both Windows 8 and Windows 7.
//!
//! 2. CriticalSection allows recursive locking while SRWLock deadlocks. The
//!    Unix implementation deadlocks so consistency is preferred. See #19962 for
//!    more details.
//!
//! 3. While CriticalSection is fair and SRWLock is not, the current Rust policy
//!    is that there are no guarantees of fairness.
//!
//! The downside of this approach, however, is that SRWLock is not available on
//! Windows XP, so we continue to have a fallback implementation where
//! CriticalSection is used and we keep track of who's holding the mutex to
//! detect recursive locks.
//!
//! For pre-Windows XP, an (even slower) fallback implementation based on
//! system-level mutexes (`CreateMutexA`) is used.

#[cfg(not(target_api_feature = "6.0.6000"))]
mod critical_section_mutex;

#[cfg(not(target_api_feature = "4.0.1381"))]
mod legacy_mutex;

use crate::cell::UnsafeCell;
use crate::mem::{self, MaybeUninit};
use crate::sync::atomic::AtomicUsize;
use crate::sys::c;

pub struct Mutex {
    lock: AtomicUsize,
    #[cfg(not(target_api_feature = "6.0.6000"))]
    held: UnsafeCell<bool>,
}

unsafe impl Send for Mutex {}
unsafe impl Sync for Mutex {}

#[derive(Clone, Copy)]
enum Kind {
    SRWLock = 1,
    #[cfg(not(target_api_feature = "6.0.6000"))]
    CriticalSection = 2,
    #[cfg(not(target_api_feature = "4.0.1381"))]
    LegacyMutex = 3,
}

#[inline]
pub unsafe fn raw<T>(m: &Mutex) -> *mut T {
    debug_assert!(mem::size_of::<T>() <= mem::size_of_val(&m.lock));
    &m.lock as *const _ as *mut _
}

impl Mutex {
    pub const fn new() -> Mutex {
        Mutex {
            // This works because SRWLOCK_INIT is 0 (wrapped in a struct), so we are also properly
            // initializing an SRWLOCK here.
            // Same for LegacyMutex (wrapping a HANDLE/zero pointer, also usized)
            lock: AtomicUsize::new(0),
            #[cfg(not(target_api_feature = "6.0.6000"))]
            held: UnsafeCell::new(false),
        }
    }
    #[inline]
    pub unsafe fn init(&mut self) {
        match kind() {
            #[cfg(not(target_api_feature = "4.0.1381"))]
            Kind::LegacyMutex => legacy_mutex::init(self),
            _ => {}
        }
    }
    pub unsafe fn lock(&self) {
        match kind() {
            Kind::SRWLock => c::AcquireSRWLockExclusive(raw(self)),
            #[cfg(not(target_api_feature = "6.0.6000"))]
            Kind::CriticalSection => critical_section_mutex::lock(self),
            #[cfg(not(target_api_feature = "4.0.1381"))]
            Kind::LegacyMutex => legacy_mutex::lock(self),
        }
    }
    pub unsafe fn try_lock(&self) -> bool {
        match kind() {
            Kind::SRWLock => c::TryAcquireSRWLockExclusive(raw(self)) != 0,
            #[cfg(not(target_api_feature = "6.0.6000"))]
            Kind::CriticalSection => critical_section_mutex::try_lock(self),
            #[cfg(not(target_api_feature = "4.0.1381"))]
            Kind::LegacyMutex => legacy_mutex::try_lock(self),
        }
    }
    pub unsafe fn unlock(&self) {
        match kind() {
            Kind::SRWLock => c::ReleaseSRWLockExclusive(raw(self)),
            #[cfg(not(target_api_feature = "6.0.6000"))]
            Kind::CriticalSection => critical_section_mutex::unlock(self),
            #[cfg(not(target_api_feature = "4.0.1381"))]
            Kind::LegacyMutex => legacy_mutex::unlock(self),
        }
    }
    pub unsafe fn destroy(&self) {
        match kind() {
            Kind::SRWLock => {}
            #[cfg(not(target_api_feature = "6.0.6000"))]
            Kind::CriticalSection => critical_section_mutex::destroy(self),
            #[cfg(not(target_api_feature = "4.0.1381"))]
            Kind::LegacyMutex => legacy_mutex::destroy(self),
        }
    }
}

cfg_if::cfg_if! {
if #[cfg(target_api_feature = "6.0.6000")] {

    #[inline(always)]
    fn kind() -> Kind {
        Kind::SRWLock
    }

} else {
    use crate::sync::atomic::Ordering;
    use crate::sys::compat;

    fn kind() -> Kind {
        static KIND: AtomicUsize = AtomicUsize::new(0);

        let val = KIND.load(Ordering::SeqCst);
        if val == Kind::SRWLock as usize {
            return Kind::SRWLock;
        } else if val == Kind::CriticalSection as usize {
            return Kind::CriticalSection;
        }

        #[cfg(not(target_api_feature = "4.0.1381"))]
        if val == Kind::LegacyMutex as usize {
            return Kind::LegacyMutex;
        }

        let ret = match compat::lookup("kernel32", "AcquireSRWLockExclusive") {
            None => {
                // critical sections exist in every win32 version, but `TryEnter` doesn't
                #[cfg(not(target_api_feature = "4.0.1381"))]
                if compat::lookup("kernel32", "TryEnterCriticalSection").is_none() {
                    return Kind::LegacyMutex;
                }

                Kind::CriticalSection
            },
            Some(..) => Kind::SRWLock,
        };
        KIND.store(ret as usize, Ordering::SeqCst);
        ret
    }
}
}

pub struct ReentrantMutex {
    inner: UnsafeCell<MaybeUninit<c::CRITICAL_SECTION>>,
}

unsafe impl Send for ReentrantMutex {}
unsafe impl Sync for ReentrantMutex {}

impl ReentrantMutex {
    pub const fn uninitialized() -> ReentrantMutex {
        ReentrantMutex { inner: UnsafeCell::new(MaybeUninit::uninit()) }
    }

    pub unsafe fn init(&self) {
        c::InitializeCriticalSection((&mut *self.inner.get()).as_mut_ptr());
    }

    pub unsafe fn lock(&self) {
        c::EnterCriticalSection((&mut *self.inner.get()).as_mut_ptr());
    }

    #[inline]
    pub unsafe fn try_lock(&self) -> bool {
        c::TryEnterCriticalSection((&mut *self.inner.get()).as_mut_ptr()) != 0
    }

    pub unsafe fn unlock(&self) {
        c::LeaveCriticalSection((&mut *self.inner.get()).as_mut_ptr());
    }

    pub unsafe fn destroy(&self) {
        c::DeleteCriticalSection((&mut *self.inner.get()).as_mut_ptr());
    }
}
