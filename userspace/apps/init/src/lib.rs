#![no_std]

use core::str;

const MAX_LINE: usize = 256;
const BIN_PREFIX: &str = "/bin/";
const MAX_ENV_VARS: usize = 16;
const MAX_ENV_LEN: usize = 511;

#[derive(Clone, Copy)]
struct EnvVar {
    key: [u8; 32],
    key_len: usize,
    val: [u8; 64],
    val_len: usize,
}

impl EnvVar {
    const fn empty() -> Self {
        Self {
            key: [0; 32],
            key_len: 0,
            val: [0; 64],
            val_len: 0,
        }
    }
}

static mut ENV: [EnvVar; MAX_ENV_VARS] = [EnvVar::empty(); MAX_ENV_VARS];

pub fn run() -> u64 {
    write_str("smultron shell (userspace)\n");
    write_str("type 'help' for commands\n");

    let mut line = [0u8; MAX_LINE];
    loop {
        write_str("smultron$ ");
        let len = read_line(&mut line);
        if !dispatch(&line[..len]) {
            return 0;
        }
    }
}

fn write_str(s: &str) {
    let _ = libos::write(1, s.as_ptr(), s.len() as u64);
}

fn read_line(buf: &mut [u8; MAX_LINE]) -> usize {
    let mut len = 0usize;
    loop {
        let mut byte = 0u8;
        if libos::read(0, &mut byte as *mut u8, 1) != 1 {
            continue;
        }

        match byte {
            b'\r' | b'\n' => {
                write_str("\n");
                return len;
            }
            8 | 127 => {
                if len > 0 {
                    len -= 1;
                    write_str("\x08 \x08");
                }
            }
            b => {
                if len < MAX_LINE {
                    buf[len] = b;
                    len += 1;
                    let _ = libos::write(1, &b as *const u8, 1);
                }
            }
        }
    }
}

fn set_env(key: &str, val: &str) {
    if key.len() > 32 || val.len() > 64 {
        write_str("error: env var too long\n");
        return;
    }

    let key_bytes = key.as_bytes();
    let val_bytes = val.as_bytes();

    unsafe {
        let mut empty_idx = None;
        for i in 0..MAX_ENV_VARS {
            if ENV[i].key_len == key.len() && &ENV[i].key[..key.len()] == key_bytes {
                ENV[i].val_len = val.len();
                ENV[i].val[..val.len()].copy_from_slice(val_bytes);
                return;
            }
            if ENV[i].key_len == 0 && empty_idx.is_none() {
                empty_idx = Some(i);
            }
        }

        if let Some(i) = empty_idx {
            ENV[i].key_len = key.len();
            ENV[i].key[..key.len()].copy_from_slice(key_bytes);
            ENV[i].val_len = val.len();
            ENV[i].val[..val.len()].copy_from_slice(val_bytes);
        } else {
            write_str("error: too many env vars\n");
        }
    }
}

fn build_env_str() -> ([u8; MAX_ENV_LEN + 1], usize) {
    let mut buf = [0u8; MAX_ENV_LEN + 1];
    let mut offset = 0;

    unsafe {
        for i in 0..MAX_ENV_VARS {
            if ENV[i].key_len > 0 {
                let pair_len = ENV[i].key_len + 1 + ENV[i].val_len + 1; // KEY=VAL\n
                if offset + pair_len > MAX_ENV_LEN {
                    break;
                }

                buf[offset..offset + ENV[i].key_len].copy_from_slice(&ENV[i].key[..ENV[i].key_len]);
                offset += ENV[i].key_len;

                buf[offset] = b'=';
                offset += 1;

                buf[offset..offset + ENV[i].val_len].copy_from_slice(&ENV[i].val[..ENV[i].val_len]);
                offset += ENV[i].val_len;

                buf[offset] = b'\n';
                offset += 1;
            }
        }
    }
    (buf, offset)
}

fn dispatch(line: &[u8]) -> bool {
    let command = str::from_utf8(line).unwrap_or("").trim();
    if command.is_empty() {
        return true;
    }

    let mut parts = command.splitn(2, char::is_whitespace);
    let cmd = parts.next().unwrap_or("");
    let args = parts.next().unwrap_or("").trim_start();

    match cmd {
        "help" => {
            write_str("commands: help, clear, exit, env, export, <binary in /bin>\n");
            true
        }
        "clear" => {
            write_str("\x1b[2J\x1b[H");
            true
        }
        "exit" => false,
        "env" => {
            let (env_buf, env_len) = build_env_str();
            let env_str = core::str::from_utf8(&env_buf[..env_len]).unwrap_or("");
            write_str(env_str);
            true
        }
        "export" => {
            if let Some((k, v)) = args.split_once('=') {
                set_env(k.trim(), v.trim());
            } else {
                write_str("usage: export KEY=VALUE\n");
            }
            true
        }
        _ => {
            if exec_from_bin(cmd, args) == u64::MAX {
                write_str("unknown command: ");
                write_str(cmd);
                write_str("\n");
            }
            true
        }
    }
}

fn exec_from_bin(cmd: &str, args: &str) -> u64 {
    let (env_buf, env_len) = build_env_str();
    let env_str = core::str::from_utf8(&env_buf[..env_len]).unwrap_or("");

    if cmd.starts_with('/') {
        return libos::exec_str_env(cmd, args, env_str);
    }

    let prefix = BIN_PREFIX.as_bytes();
    let cmd_bytes = cmd.as_bytes();
    let total = prefix.len() + cmd_bytes.len();
    if total == 0 || total > 63 {
        return u64::MAX;
    }

    let mut path_buf = [core::mem::MaybeUninit::<u8>::uninit(); 64];

    let mut i = 0;
    while i < prefix.len() {
        path_buf[i].write(prefix[i]);
        i += 1;
    }

    let mut j = 0;
    while j < cmd_bytes.len() {
        path_buf[i + j].write(cmd_bytes[j]);
        j += 1;
    }

    let path_slice = unsafe { core::slice::from_raw_parts(path_buf.as_ptr() as *const u8, total) };
    let path = match core::str::from_utf8(path_slice) {
        Ok(p) => p,
        Err(_) => return u64::MAX,
    };
    libos::exec_str_env(path, args, env_str)
}
