#![cfg_attr(test, allow(dead_code))]

use crate::sys::c::SetThreadStackGuarantee;

pub struct Handler;

cfg_if::cfg_if! {
if #[cfg(target_api_feature = "5.2.3790")] {
    impl Handler {
        pub unsafe fn new() -> Handler {
            if SetThreadStackGuarantee(&mut 0x5000) == 0 {
                panic!("failed to reserve stack space for exception handling");
            }
            Handler
        }
    }
} else {
    impl Handler {
        pub unsafe fn new() -> Handler {
            // This API isn't available on XP, so don't panic in that case and just
            // pray it works out ok.
            if SetThreadStackGuarantee(&mut 0x5000) == 0 {
                if c::GetLastError() as u32 != c::ERROR_CALL_NOT_IMPLEMENTED as u32 {
                    panic!("failed to reserve stack space for exception handling");
                }
            }
            Handler
        }
    }
}
}

use crate::sys::c;
use crate::sys_common::util::report_overflow;

extern "system" fn vectored_handler(ExceptionInfo: *mut c::EXCEPTION_POINTERS) -> c::LONG {
    unsafe {
        let rec = &(*(*ExceptionInfo).ExceptionRecord);
        let code = rec.ExceptionCode;

        if code == c::EXCEPTION_STACK_OVERFLOW {
            report_overflow();
        }
        c::EXCEPTION_CONTINUE_SEARCH
    }
}

pub unsafe fn init() {
    if c::AddVectoredExceptionHandler(0, vectored_handler).is_null() {
        if c::GetLastError() as u32 != c::ERROR_CALL_NOT_IMPLEMENTED as u32 {
            panic!("failed to install exception handler");
        }
    }
    // Set the thread stack guarantee for the main thread.
    let _h = Handler::new();
}

pub unsafe fn cleanup() {}
