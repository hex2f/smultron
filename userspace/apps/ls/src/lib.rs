#![no_std]

pub fn run(cwd: &str) {
    let mut raw = [core::mem::MaybeUninit::<u8>::uninit(); 1024];
    let buf = unsafe { core::slice::from_raw_parts_mut(raw.as_mut_ptr() as *mut u8, raw.len()) };
    let len = libos::list_dir(cwd, buf);

    if len == 0 || len > buf.len() {
        return;
    }

    let mut i = 0;
    while i < len {
        let mut j = i;
        while j < len && buf[j] != 0 {
            j += 1;
        }

        if j > i {
            let _ = libos::write(1, buf[i..].as_ptr(), (j - i) as u64);
            let _ = libos::write(1, b"\n".as_ptr(), 1);
        }

        i = j + 1;
    }
}
