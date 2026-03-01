#![no_std]

use core::str;

const MAX_LINE: usize = 256;
const BIN_PREFIX: &str = "/bin/";

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
            write_str("commands: help, clear, exit, <binary in /bin>\n");
            true
        }
        "clear" => {
            write_str("\x1b[2J\x1b[H");
            true
        }
        "exit" => false,
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
    if cmd.starts_with('/') {
        return libos::exec_str(cmd, args);
    }

    let mut path_buf = [0u8; 64];
    let prefix = BIN_PREFIX.as_bytes();
    let cmd_bytes = cmd.as_bytes();
    let total = prefix.len() + cmd_bytes.len();
    if total == 0 || total > 63 {
        return u64::MAX;
    }

    path_buf[..prefix.len()].copy_from_slice(prefix);
    path_buf[prefix.len()..total].copy_from_slice(cmd_bytes);
    let path = match core::str::from_utf8(&path_buf[..total]) {
        Ok(p) => p,
        Err(_) => return u64::MAX,
    };
    libos::exec_str(path, args)
}
