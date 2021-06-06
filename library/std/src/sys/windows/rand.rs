use crate::io;
use crate::mem;
use crate::sys::c;

#[cfg(not(target_vendor = "uwp"))]
pub fn hashmap_random_keys() -> (u64, u64) {
    let mut v;
    if c::SystemFunction036::available() {
        v = (0, 0);
        let ret = unsafe {
            c::RtlGenRandom(&mut v as *mut _ as *mut u8, mem::size_of_val(&v) as c::ULONG)
        };
        if ret == 0 {
            panic!("couldn't generate random bytes: {}", io::Error::last_os_error());
        }
    } else {
        unsafe {
            let tickCount = c::GetTickCount();
            let id = c::GetCurrentProcessId();
            let mut file_time: c::FILETIME = crate::mem::zeroed();
            c::GetSystemTimeAsFileTime(&mut file_time as *mut _);

            v = (
                (file_time.dwHighDateTime as u64) << 32 | tickCount as u64,
                (id as u64) << 32 | file_time.dwLowDateTime as u64,
            )
        }
    }
    v
}

#[cfg(target_vendor = "uwp")]
pub fn hashmap_random_keys() -> (u64, u64) {
    use crate::ptr;

    let mut v = (0, 0);
    let ret = unsafe {
        c::BCryptGenRandom(
            ptr::null_mut(),
            &mut v as *mut _ as *mut u8,
            mem::size_of_val(&v) as c::ULONG,
            c::BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    };
    if ret != 0 {
        panic!("couldn't generate random bytes: {}", io::Error::last_os_error());
    }
    return v;
}
