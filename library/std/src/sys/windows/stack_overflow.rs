#![cfg_attr(test, allow(dead_code))]

use crate::sys::c;
use crate::sys_common::util::report_overflow;

pub struct Handler;

impl Handler {
    pub unsafe fn new() -> Handler {
        if c::SetThreadStackGuarantee::available() {
            if c::SetThreadStackGuarantee(&mut 0x5000) == 0 {
                panic!("failed to reserve stack space for exception handling");
            }
        }
        Handler
    }
}

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
    if !c::AddVectoredExceptionHandler::available() {
        return;
    }

    if c::AddVectoredExceptionHandler(0, vectored_handler).is_null() {
        panic!("failed to install exception handler");
    }
    // Set the thread stack guarantee for the main thread.
    let _h = Handler::new();
}
