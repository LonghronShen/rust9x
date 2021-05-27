use crate::cell::UnsafeCell;
use crate::io;
use crate::mem::{size_of, size_of_val};
use crate::ptr;
use crate::sys::mutex::compat::{MutexKind, MUTEX_KIND};
use crate::sys::mutex::Mutex;
use crate::sys::os;
use crate::sys::{c, cvt};
use crate::time::Duration;

pub struct Condvar {
    inner: UnsafeCell<usize>,
}

pub type MovableCondvar = Condvar;

unsafe impl Send for Condvar {}
unsafe impl Sync for Condvar {}

impl Condvar {
    pub const fn new() -> Condvar {
        // a `CONDITION_VARIABLE` (SRW impl) is `usize`-sized, and the correct
        // `CONDITION_VARIABLE_INIT` value happens to be zeroed. this happens to also be a valid
        // (zero) init for the fallback `HANDLE`.
        Condvar { inner: UnsafeCell::new(0) }
    }

    #[inline]
    pub unsafe fn init(&mut self) {
        debug_assert_eq!(size_of::<usize>(), size_of_val(&c::CONDITION_VARIABLE_INIT));
        match MUTEX_KIND {
            MutexKind::SrwLock => {}
            MutexKind::CriticalSection | MutexKind::Legacy => {
                let evt_handle = c::CreateEventA(
                    ptr::null_mut(),
                    c::TRUE, // manual reset event
                    c::FALSE,
                    ptr::null(),
                );

                if evt_handle.is_null() {
                    panic!("failed creating event: {}", io::Error::last_os_error());
                }

                *self.inner.get() = evt_handle as usize;
            }
        }
    }

    #[inline]
    pub unsafe fn wait(&self, mutex: &Mutex) {
        match MUTEX_KIND {
            MutexKind::SrwLock => {
                let r = c::SleepConditionVariableSRW(
                    self.inner.get().cast(),
                    mutex.raw(),
                    c::INFINITE,
                    0,
                );
                debug_assert!(r != 0);
            }
            MutexKind::CriticalSection | MutexKind::Legacy => {
                mutex.unlock();
                if (c::WaitForSingleObject((*self.inner.get()) as c::HANDLE, c::INFINITE))
                    != c::WAIT_OBJECT_0
                {
                    panic!("event wait failed: {}", io::Error::last_os_error())
                }
                mutex.lock();
            }
        }
    }

    pub unsafe fn wait_timeout(&self, mutex: &Mutex, dur: Duration) -> bool {
        match MUTEX_KIND {
            MutexKind::SrwLock => {
                let r = c::SleepConditionVariableSRW(
                    self.inner.get().cast(),
                    mutex.raw(),
                    super::dur2timeout(dur),
                    0,
                );
                if r == 0 {
                    debug_assert_eq!(os::errno() as usize, c::ERROR_TIMEOUT as usize);
                    false
                } else {
                    true
                }
            }
            MutexKind::CriticalSection | MutexKind::Legacy => {
                mutex.unlock();
                let ret = match c::WaitForSingleObject(
                    (*self.inner.get()) as c::HANDLE,
                    super::dur2timeout(dur),
                ) {
                    c::WAIT_OBJECT_0 => true,
                    c::WAIT_TIMEOUT => false,
                    _ => panic!("event wait failed: {}", io::Error::last_os_error()),
                };
                mutex.lock();
                ret
            }
        }
    }

    #[inline]
    pub unsafe fn notify_one(&self) {
        match MUTEX_KIND {
            MutexKind::SrwLock => c::WakeConditionVariable(self.inner.get().cast()),
            MutexKind::CriticalSection | MutexKind::Legacy => {
                // this currently wakes up all threads, but spurious wakeups are allowed, so this is
                // at most a performance issue
                cvt(c::PulseEvent((*self.inner.get()) as c::HANDLE)).unwrap();
            }
        }
    }

    #[inline]
    pub unsafe fn notify_all(&self) {
        match MUTEX_KIND {
            MutexKind::SrwLock => c::WakeAllConditionVariable(self.inner.get().cast()),
            MutexKind::CriticalSection | MutexKind::Legacy => {
                cvt(c::PulseEvent((*self.inner.get()) as c::HANDLE)).unwrap();
            }
        };
    }

    pub unsafe fn destroy(&self) {
        match MUTEX_KIND {
            MutexKind::SrwLock => (),
            MutexKind::CriticalSection | MutexKind::Legacy => {
                cvt(c::CloseHandle((*self.inner.get()) as c::HANDLE)).unwrap();
            }
        };
    }
}
