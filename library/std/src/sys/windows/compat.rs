//! A "compatibility layer" for supporting older versions of Windows
//!
//! The standard library uses some Windows API functions that are not present
//! on older versions of Windows.  (Note that the oldest version of Windows
//! that Rust supports is Windows 7 (client) and Windows Server 2008 (server).)
//! This module implements a form of delayed DLL import binding, using
//! `GetModuleHandle` and `GetProcAddress` to look up DLL entry points at
//! runtime.
//!
//! This implementation uses a static initializer to look up the DLL entry
//! points. The CRT (C runtime) executes static initializers before `main`
//! is called (for binaries) and before `DllMain` is called (for DLLs).
//! This is the ideal time to look up DLL imports, because we are guaranteed
//! that no other threads will attempt to call these entry points. Thus,
//! we can look up the imports and store them in `static mut` fields
//! without any synchronization.
//!
//! This has an additional advantage: Because the DLL import lookup happens
//! at module initialization, the cost of these lookups is deterministic,
//! and is removed from the code paths that actually call the DLL imports.
//! That is, there is no unpredictable "cache miss" that occurs when calling
//! a DLL import. For applications that benefit from predictable delays,
//! this is a benefit. This also eliminates the comparison-and-branch
//! from the hot path.
//!
//! Currently, the standard library uses only a small number of dynamic
//! DLL imports. If this number grows substantially, then the cost of
//! performing all of the lookups at initialization time might become
//! substantial.
//!
//! The mechanism of registering a static initializer with the CRT is
//! documented in
//! [CRT Initialization](https://docs.microsoft.com/en-us/cpp/c-runtime-library/crt-initialization?view=msvc-160).
//! It works by contributing a global symbol to the `.CRT$XCU` section.
//! The linker builds a table of all static initializer functions.
//! The CRT startup code then iterates that table, calling each
//! initializer function.
//!
//! # **WARNING!!*
//! The environment that a static initializer function runs in is highly
//! constrained. There are **many** restrictions on what static initializers
//! can safely do. Static initializer functions **MUST NOT** do any of the
//! following (this list is not comprehensive):
//! * touch any other static field that is used by a different static
//!   initializer, because the order that static initializers run in
//!   is not defined.
//! * call `LoadLibrary` or any other function that acquires the DLL
//!   loader lock.
//! * call any Rust function or CRT function that touches any static
//!   (global) state.

use crate::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use crate::sys::c;

macro_rules! compat_fn {
    ($module:literal: $(
        $(#[$meta:meta])*
        pub fn $symbol:ident($($argname:ident: $argtype:ty),*) -> $rettype:ty $fallback_body:block
    )*) => ($(
        $(#[$meta])*
        pub mod $symbol {
            #[allow(unused_imports)]
            use super::*;
            use crate::mem;

            type F = unsafe extern "system" fn($($argtype),*) -> $rettype;

            /// Points to the DLL import, or the fallback function.
            ///
            /// This static can be an ordinary, unsynchronized, mutable static because
            /// we guarantee that all of the writes finish during CRT initialization,
            /// and all of the reads occur after CRT initialization.
            static mut PTR: F = fallback;
            static mut AVAILABLE: bool = false;

            /// This symbol is what allows the CRT to find the `init` function and call it.
            /// It is marked `#[used]` because otherwise Rust would assume that it was not
            /// used, and would remove it.
            #[used]
            #[link_section = ".CRT$XCU"]
            static INIT_TABLE_ENTRY: unsafe extern "C" fn() = init;

            unsafe extern "C" fn init() {
                // There is no locking here. This code is executed before main() is entered, and
                // is guaranteed to be single-threaded.
                //
                // DO NOT do anything interesting or complicated in this function! DO NOT call
                // any Rust functions or CRT functions, if those functions touch any global state,
                // because this function runs during global initialization. For example, DO NOT
                // do any dynamic allocation, don't call LoadLibrary, etc.

                let symbol_name: *const u8 = concat!(stringify!($symbol), "\0").as_ptr();

                let unicows_module_name: *const u8 = concat!("unicows", "\0").as_ptr();
                let unicows_handle = $crate::sys::c::GetModuleHandleA(unicows_module_name as *const i8);
                if !unicows_handle.is_null() {
                    match $crate::sys::c::GetProcAddress(unicows_handle, symbol_name as *const i8) as usize {
                        0 => {}
                        n => {
                            PTR = mem::transmute::<usize, F>(n);
                            AVAILABLE = true;
                            return;
                        }
                    }
                }

                let module_name: *const u8 = concat!($module, "\0").as_ptr();
                let module_handle = $crate::sys::c::GetModuleHandleA(module_name as *const i8);
                if !module_handle.is_null() {
                    match $crate::sys::c::GetProcAddress(module_handle, symbol_name as *const i8) as usize {
                        0 => {}
                        n => {
                            PTR = mem::transmute::<usize, F>(n);
                            AVAILABLE = true;
                        }
                    }
                }
            }

            #[allow(dead_code)]
            pub fn option() -> Option<F> {
                unsafe {
                    if AVAILABLE {
                        Some(PTR)
                    } else {
                        None
                    }
                }
            }

            #[allow(dead_code)]
            #[inline(always)]
            pub fn available() -> bool {
                unsafe { AVAILABLE }
            }

            #[allow(dead_code)]
            #[inline(always)]
            pub unsafe fn call($($argname: $argtype),*) -> $rettype {
                PTR($($argname),*)
            }

            #[allow(dead_code)]
            unsafe extern "system" fn fallback(
                $(#[allow(unused_variables)] $argname: $argtype),*
            ) -> $rettype {
                $fallback_body
            }
        }

        $(#[$meta])*
        pub use $symbol::call as $symbol;
    )*)
}

macro_rules! compat_fn_lazy {
    ($module:literal:{unicows: $unicows:literal, load: $load:literal}: $(
        $(#[$meta:meta])*
        pub fn $symbol:ident($($argname:ident: $argtype:ty),*) -> $rettype:ty $fallback_body:block
    )*) => ($(
        $(#[$meta])*
        pub mod $symbol {
            #[allow(unused_imports)]
            use super::*;
            use crate::sync::atomic::{AtomicUsize, AtomicBool, Ordering};
            use crate::mem;

            type F = unsafe extern "system" fn($($argtype),*) -> $rettype;

            static PTR: AtomicUsize = AtomicUsize::new(0);
            static AVAILABLE: AtomicBool = AtomicBool::new(false);

            #[allow(dead_code)]
            fn load() -> usize {
                unsafe {
                    crate::sys::compat::store_func(
                        &PTR,
                        &AVAILABLE,
                        concat!($module, "\0").as_ptr(),
                        concat!(stringify!($symbol), "\0").as_ptr(),
                        fallback as usize,
                        $unicows,
                        $load
                    )
                }
            }

            #[allow(dead_code)]
            pub fn option() -> Option<F> {
                let addr = match PTR.load(Ordering::SeqCst) {
                    0 => load(),
                    n => n,
                };

                unsafe {
                    if AVAILABLE.load(Ordering::SeqCst) {
                        Some(mem::transmute::<usize, F>(addr))
                    } else {
                        None
                    }
                }
            }

            #[allow(dead_code)]
            pub fn available() -> bool {
                if PTR.load(Ordering::SeqCst) == 0 {
                    load();
                }
                AVAILABLE.load(Ordering::SeqCst)
            }

            #[allow(dead_code)]
            pub unsafe fn call($($argname: $argtype),*) -> $rettype {
                let addr = match PTR.load(Ordering::SeqCst) {
                    0 => load(),
                    n => n,
                };
                mem::transmute::<usize, F>(addr)($($argname),*)
            }

            #[allow(dead_code)]
            unsafe extern "system" fn fallback(
                $(#[allow(unused_variables)] $argname: $argtype),*
            ) -> $rettype {
                $fallback_body
            }
        }

        $(#[$meta])*
        pub use $symbol::call as $symbol;
    )*)
}

const UNICOWS_MODULE_NAME: &str = "unicows\0";

unsafe fn lookup(
    module: *const u8,
    symbol: *const u8,
    check_unicows: bool,
    load_library: bool,
) -> Option<usize> {
    if check_unicows {
        let unicows_handle = c::GetModuleHandleA(UNICOWS_MODULE_NAME.as_ptr() as *const i8);
        if !unicows_handle.is_null() {
            match c::GetProcAddress(unicows_handle, symbol as *const i8) as usize {
                0 => {}
                n => {
                    return Some(n);
                }
            }
        }
    }

    let handle = if load_library {
        c::LoadLibraryA(module as *const i8)
    } else {
        c::GetModuleHandleA(module as *const i8)
    };

    if handle.is_null() {
        return None;
    }

    match c::GetProcAddress(handle, symbol as *const i8) as usize {
        0 => None,
        n => Some(n),
    }
}

pub unsafe fn store_func(
    ptr: &AtomicUsize,
    available: &AtomicBool,
    module: *const u8,
    symbol: *const u8,
    fallback: usize,
    check_unicows: bool,
    load_library: bool,
) -> usize {
    let value = match lookup(module, symbol, check_unicows, load_library) {
        Some(value) => {
            available.store(true, Ordering::SeqCst);
            value
        }
        None => fallback,
    };

    ptr.store(value, Ordering::SeqCst);
    value
}
