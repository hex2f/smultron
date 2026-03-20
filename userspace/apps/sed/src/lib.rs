#![no_std]

const MAX_TOKENS: usize = 16;
const MAX_PATTERN: usize = 128;
const MAX_REPLACEMENT: usize = 128;
const MAX_IO_BUF: usize = 4096;

struct Script {
    pattern: [u8; MAX_PATTERN],
    pattern_len: usize,
    replacement: [u8; MAX_REPLACEMENT],
    replacement_len: usize,
    global: bool,
}

pub fn run(args: &str, cwd: &str) {
    let mut tokens = [""; MAX_TOKENS];
    let mut token_count = 0usize;
    for tok in args.split_whitespace() {
        if token_count >= MAX_TOKENS {
            write_str("sed: too many arguments\n");
            return;
        }
        tokens[token_count] = tok;
        token_count += 1;
    }

    if token_count == 0 {
        write_str("usage: sed s/old/new/[g] [file...]\n");
        return;
    }

    let Some(script) = parse_script(tokens[0]) else {
        write_str("sed: unsupported script (expected s<delim>old<delim>new<delim>[g])\n");
        return;
    };

    if token_count == 1 {
        run_on_stdin(&script);
        return;
    }

    let mut i = 1usize;
    while i < token_count {
        run_on_file(&script, tokens[i], cwd);
        i += 1;
    }
}

fn run_on_stdin(script: &Script) {
    let mut in_raw = [core::mem::MaybeUninit::<u8>::uninit(); MAX_IO_BUF];
    let mut out_raw = [core::mem::MaybeUninit::<u8>::uninit(); MAX_IO_BUF];
    let in_buf = unsafe { core::slice::from_raw_parts_mut(in_raw.as_mut_ptr() as *mut u8, in_raw.len()) };
    let out_buf =
        unsafe { core::slice::from_raw_parts_mut(out_raw.as_mut_ptr() as *mut u8, out_raw.len()) };

    loop {
        let n = libos::read(0, in_buf.as_mut_ptr(), in_buf.len() as u64);
        if n == u64::MAX {
            write_str("sed: read error\n");
            return;
        }
        if n == 0 {
            break;
        }

        let len = substitute(
            &in_buf[..n as usize],
            out_buf,
            &script.pattern[..script.pattern_len],
            &script.replacement[..script.replacement_len],
            script.global,
        );
        let _ = libos::write(1, out_buf.as_ptr(), len as u64);
    }
}

fn run_on_file(script: &Script, path_arg: &str, cwd: &str) {
    let mut path_raw = [core::mem::MaybeUninit::<u8>::uninit(); 128];
    let path = match resolve_path(path_arg, cwd, &mut path_raw) {
        Some(p) => p,
        None => {
            write_str("sed: path too long\n");
            return;
        }
    };

    let mut in_raw = [core::mem::MaybeUninit::<u8>::uninit(); MAX_IO_BUF];
    let mut out_raw = [core::mem::MaybeUninit::<u8>::uninit(); MAX_IO_BUF];
    let in_buf = unsafe { core::slice::from_raw_parts_mut(in_raw.as_mut_ptr() as *mut u8, in_raw.len()) };
    let out_buf =
        unsafe { core::slice::from_raw_parts_mut(out_raw.as_mut_ptr() as *mut u8, out_raw.len()) };

    let Some(n) = libos::read_file(path, in_buf) else {
        write_str("sed: cannot read ");
        write_str(path);
        write_str("\n");
        return;
    };

    let len = substitute(
        &in_buf[..n],
        out_buf,
        &script.pattern[..script.pattern_len],
        &script.replacement[..script.replacement_len],
        script.global,
    );
    let _ = libos::write(1, out_buf.as_ptr(), len as u64);
}

fn substitute(input: &[u8], out: &mut [u8], pattern: &[u8], replacement: &[u8], global: bool) -> usize {
    if pattern.is_empty() {
        let copy_len = core::cmp::min(input.len(), out.len());
        out[..copy_len].copy_from_slice(&input[..copy_len]);
        return copy_len;
    }

    let mut in_i = 0usize;
    let mut out_i = 0usize;
    while in_i < input.len() && out_i < out.len() {
        let matched = in_i + pattern.len() <= input.len() && &input[in_i..in_i + pattern.len()] == pattern;
        if matched {
            let copy_len = core::cmp::min(replacement.len(), out.len() - out_i);
            out[out_i..out_i + copy_len].copy_from_slice(&replacement[..copy_len]);
            out_i += copy_len;
            in_i += pattern.len();
            if !global {
                while in_i < input.len() && out_i < out.len() {
                    out[out_i] = input[in_i];
                    out_i += 1;
                    in_i += 1;
                }
                break;
            }
        } else {
            out[out_i] = input[in_i];
            out_i += 1;
            in_i += 1;
        }
    }
    out_i
}

fn parse_script(script: &str) -> Option<Script> {
    let bytes = script.as_bytes();
    if bytes.len() < 4 || bytes[0] != b's' {
        return None;
    }
    let delim = bytes[1];
    if delim == 0 {
        return None;
    }

    let mut pattern = [0u8; MAX_PATTERN];
    let mut replacement = [0u8; MAX_REPLACEMENT];
    let (pat_len, idx_after_pat) = parse_delimited(bytes, 2, delim, &mut pattern)?;
    let (rep_len, idx_after_rep) = parse_delimited(bytes, idx_after_pat, delim, &mut replacement)?;

    let flags = &bytes[idx_after_rep..];
    let global = match flags {
        b"" => false,
        b"g" => true,
        _ => return None,
    };

    Some(Script {
        pattern,
        pattern_len: pat_len,
        replacement,
        replacement_len: rep_len,
        global,
    })
}

fn parse_delimited(input: &[u8], start: usize, delim: u8, out: &mut [u8]) -> Option<(usize, usize)> {
    let mut i = start;
    let mut len = 0usize;
    let mut escaped = false;
    while i < input.len() {
        let b = input[i];
        if escaped {
            if len >= out.len() {
                return None;
            }
            out[len] = b;
            len += 1;
            escaped = false;
            i += 1;
            continue;
        }
        if b == b'\\' {
            escaped = true;
            i += 1;
            continue;
        }
        if b == delim {
            return Some((len, i + 1));
        }
        if len >= out.len() {
            return None;
        }
        out[len] = b;
        len += 1;
        i += 1;
    }
    None
}

fn resolve_path<'a>(
    path: &'a str,
    cwd: &str,
    out: &'a mut [core::mem::MaybeUninit<u8>; 128],
) -> Option<&'a str> {
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
