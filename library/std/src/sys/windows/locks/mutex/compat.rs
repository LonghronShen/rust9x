use crate::convert::AsRef;
use crate::sync::atomic::{AtomicUsize, Ordering};
use crate::sys::c;

/// Taken from the [once-removed](https://github.com/rust-lang/rust/pull/81250) Windows XP compatible mutex implementation
#[inline(always)]
pub fn atomic_boxed_init<T>(
    storage: &AtomicUsize,
    init: unsafe fn() -> Box<T>,
    destroy: unsafe fn(&T),
) -> *mut T {
    match storage.load(Ordering::SeqCst) {
        0 => {}
        n => return n as *mut _,
    }
    let re = unsafe { init() };
    let re = Box::into_raw(re);
    match storage.compare_exchange(0, re as usize, Ordering::SeqCst, Ordering::SeqCst) {
        Ok(_) => re,
        Err(n) => {
            unsafe { destroy(Box::from_raw(re).as_ref()) };
            n as *mut _
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum MutexKind {
    /// Win 7+ (Vista doesn't support the `Try*` APIs)
    SrwLock,
    /// NT 4+ (9x/ME/NT3.x support critical sections, but don't support `TryEnterCriticalSection`)
    CriticalSection,
    /// Good ol' `CreateMutex`
    Legacy,
}

pub static mut MUTEX_KIND: MutexKind = MutexKind::SrwLock;

/// See the main windows compat.rs on what this is
#[used]
// Makes sure this initializer runs after regular global/XCU initializers, but before any other MSVCRT
// initializers. This is needed so that all the compat API info is initialized here.
#[link_section = ".CRT$XCU_AFTER"]
static INIT_TABLE_ENTRY: unsafe extern "C" fn() = init;

unsafe extern "C" fn init() {
    MUTEX_KIND = if c::TryAcquireSRWLockExclusive::available() {
        MutexKind::SrwLock
    } else if c::TryEnterCriticalSection::available() {
        MutexKind::CriticalSection
    } else {
        MutexKind::Legacy
    };
}
