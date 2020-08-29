pub unsafe fn RtlGenRandom(
    RandomBuffer: *mut u8,
    RandomBufferLength: super::ULONG,
) -> super::BOOLEAN {
    let tickCount = super::GetTickCount();
    let mut file_time: super::FILETIME = crate::mem::zeroed();
    super::GetSystemTimeAsFileTime(&mut file_time as *mut _);
    let id = super::GetCurrentProcessId();

    let mut state = XorwowState {
        shift: [tickCount, id, file_time.dwLowDateTime, file_time.dwHighDateTime],
        counter: 0,
    };

    let len_usize = RandomBufferLength as usize;
    let rest = len_usize % 4;
    let u32s = len_usize / 4;

    let buf = crate::slice::from_raw_parts_mut(RandomBuffer, len_usize);

    for i in 0..u32s {
        let offset = i * 4;
        buf[offset..offset + 4].copy_from_slice(&xorwow(&mut state));
    }

    if rest > 0 {
        buf[len_usize - rest..len_usize].copy_from_slice(&xorwow(&mut state)[..rest]);
    }

    1
}

struct XorwowState {
    shift: [u32; 4],
    counter: u32,
}

fn xorwow(state: &mut XorwowState) -> [u8; 4] {
    let mut t = state.shift[3];

    let s = state.shift[0];
    state.shift[3] = state.shift[2];
    state.shift[2] = state.shift[1];
    state.shift[1] = s;

    t ^= t.wrapping_shr(2);
    t ^= t.wrapping_shl(1);
    t ^= s ^ s.wrapping_shl(4);
    state.shift[0] = t;

    state.counter = state.counter.wrapping_add(362437);
    return t.wrapping_add(state.counter).to_ne_bytes();
}
