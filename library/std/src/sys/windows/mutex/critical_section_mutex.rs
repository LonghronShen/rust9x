use crate::sync::atomic::Ordering;
use crate::sys::mutex::{Mutex, ReentrantMutex};

#[inline]
pub unsafe fn lock(mutex: &Mutex) {
    let re = mutex.remutex();
    (*re).lock();
    if !mutex.flag_locked() {
        (*re).unlock();
        panic!("cannot recursively lock a mutex");
    }
}

#[inline]
pub unsafe fn try_lock(mutex: &Mutex) -> bool {
    let re = mutex.remutex();
    if !(*re).try_lock() {
        false
    } else if mutex.flag_locked() {
        true
    } else {
        (*re).unlock();
        false
    }
}

#[inline]
pub unsafe fn unlock(mutex: &Mutex) {
    *mutex.held.get() = false;
    (*mutex.remutex()).unlock();
}

#[inline]
pub unsafe fn destroy(mutex: &Mutex) {
    match mutex.lock.load(Ordering::SeqCst) {
        0 => {}
        n => {
            Box::from_raw(n as *mut ReentrantMutex).destroy();
        }
    }
}

trait CriticalSectionToMutex {
    unsafe fn remutex(&self) -> *mut ReentrantMutex;
    unsafe fn flag_locked(&self) -> bool;
}

impl CriticalSectionToMutex for Mutex {
    unsafe fn remutex(&self) -> *mut ReentrantMutex {
        match self.lock.load(Ordering::SeqCst) {
            0 => {}
            n => return n as *mut _,
        }
        let re = box ReentrantMutex::uninitialized();
        re.init();
        let re = Box::into_raw(re);
        match self.lock.compare_and_swap(0, re as usize, Ordering::SeqCst) {
            0 => re,
            n => {
                Box::from_raw(re).destroy();
                n as *mut _
            }
        }
    }

    unsafe fn flag_locked(&self) -> bool {
        if *self.held.get() {
            false
        } else {
            *self.held.get() = true;
            true
        }
    }
}
