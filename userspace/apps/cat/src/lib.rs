#![no_std]

pub fn run(args: &str, cwd: &str) {
    let path_arg = args.trim();
    if path_arg.is_empty() {
        // Stdin mode: used by shell piping/redirection (`cmd | cat`, `cat < file`).
        let mut buf = [0u8; 256];
        loop {
            let n = libos::read(0, buf.as_mut_ptr(), buf.len() as u64);
            if n == 0 || n == u64::MAX {
                break;
            }
            let _ = libos::write(1, buf.as_ptr(), n);
        }
        return;
    }

    let mut resolved_storage = [core::mem::MaybeUninit::<u8>::uninit(); 128];
    let resolved = match resolve_path(path_arg, cwd, &mut resolved_storage) {
        Some(p) => p,
        None => {
            write_str("cat: path too long\n");
            return;
        }
    };

    let mut raw = [core::mem::MaybeUninit::<u8>::uninit(); 4096];
    let buf = unsafe { core::slice::from_raw_parts_mut(raw.as_mut_ptr() as *mut u8, raw.len()) };
    match libos::read_file(resolved, buf) {
        Some(len) => {
            if len == 0 {
                return;
            }
            let _ = libos::write(1, buf.as_ptr(), len as u64);
            if buf[len - 1] != b'\n' {
                let _ = libos::write(1, b"\n".as_ptr(), 1);
            }
        }
        None => {
            write_str("cat: cannot read ");
            write_str(resolved);
            write_str("\n");
        }
    }
}

fn resolve_path<'a>(path: &'a str, cwd: &str, out: &'a mut [core::mem::MaybeUninit<u8>; 128]) -> Option<&'a str> {
    if path.starts_with('/') {
        return Some(path);
    }

    let cwd_bytes = cwd.as_bytes();
    let path_bytes = path.as_bytes();
    let mut len = 0usize;

    if cwd == "/" {
        out[len].write(b'/');
        len += 1;
    } else {
        if cwd_bytes.len() + 1 > out.len() {
            return None;
        }
        let mut i = 0usize;
        while i < cwd_bytes.len() {
            out[len].write(cwd_bytes[i]);
            len += 1;
            i += 1;
        }
        if len == 0 || cwd_bytes[cwd_bytes.len() - 1] != b'/' {
            out[len].write(b'/');
            len += 1;
        }
    }

    if len + path_bytes.len() > out.len() {
        return None;
    }
    let mut i = 0usize;
    while i < path_bytes.len() {
        out[len + i].write(path_bytes[i]);
        i += 1;
    }
    len += path_bytes.len();

    let bytes = unsafe { core::slice::from_raw_parts(out.as_ptr() as *const u8, len) };
    core::str::from_utf8(bytes).ok()
}

fn write_str(s: &str) {
    let _ = libos::write(1, s.as_ptr(), s.len() as u64);
}
