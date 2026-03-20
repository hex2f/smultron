use crate::serial_println;
use crate::{serial, serial_print};
use core::slice;

#[repr(u64)]
pub enum SyscallNumber {
    Read = 0,
    Write = 1,
    Open = 2,
    Fork = 57,
    Execve = 59,
    Exit = 60,
    ReadDir = 78,
    ReadFile = 79,
    ExecIo = 80,
    WriteFile = 81,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct ExecIoRequest {
    path_ptr: *const u8,
    args_ptr: *const u8,
    env_ptr: *const u8,
    stdin_ptr: *const u8,
    stdin_len: u64,
    stdout_ptr: *mut u8,
    stdout_cap: u64,
    status_ptr: *mut u64,
}

pub fn init() {
    // Placeholder: full STAR/LSTAR/EFER setup to be completed with real ring transitions.
    serial_println!("[ok] syscall MSR init scaffolded");
}

#[no_mangle]
pub extern "C" fn smultron_syscall_gate(nr: u64, arg0: u64, arg1: u64, arg2: u64) -> u64 {
    dispatch(nr, arg0, arg1, arg2)
}

pub fn dispatch(nr: u64, arg0: u64, arg1: u64, arg2: u64) -> u64 {
    match nr {
        x if x == SyscallNumber::Read as u64 => {
            let fd = arg0;
            if fd != 0 || arg1 == 0 {
                return u64::MAX;
            }

            let len = arg2 as usize;
            if len == 0 {
                return 0;
            }

            let buf = unsafe { slice::from_raw_parts_mut(arg1 as *mut u8, len) };
            let mut count = 0usize;
            while count < len {
                match crate::process::take_stdin_byte() {
                    crate::process::StdinByte::Byte(byte) => {
                        buf[count] = byte;
                        count += 1;
                    }
                    crate::process::StdinByte::Eof => break,
                    crate::process::StdinByte::Unavailable => {
                        if let Some(byte) =
                            serial::try_read_byte().or_else(crate::keyboard::try_read_byte)
                        {
                            buf[count] = byte;
                            count += 1;
                            if byte == b'\n' || byte == b'\r' {
                                break;
                            }
                        } else {
                            core::hint::spin_loop();
                        }
                    }
                }
            }
            count as u64
        }
        x if x == SyscallNumber::Write as u64 => {
            let fd = arg0;
            if (fd != 1 && fd != 2) || arg1 == 0 {
                return u64::MAX;
            }

            let len = arg2 as usize;
            if len == 0 {
                return 0;
            }

            let buf = unsafe { slice::from_raw_parts(arg1 as *const u8, len) };
            if crate::process::capture_stdout(buf) {
                return len as u64;
            }
            for &byte in buf {
                serial_print!("{}", byte as char);
                crate::vga_buffer::write_byte(byte);
            }
            len as u64
        }
        x if x == SyscallNumber::Execve as u64 => {
            if arg0 == 0 {
                return u64::MAX;
            }

            let mut path_buf = [core::mem::MaybeUninit::<u8>::uninit(); 64];
            let path_len = match unsafe { copy_cstr_uninit(arg0 as *const u8, &mut path_buf) } {
                Some(len) => len,
                None => return u64::MAX,
            };
            let path_slice =
                unsafe { slice::from_raw_parts(path_buf.as_ptr() as *const u8, path_len) };
            let path = core::str::from_utf8(path_slice).unwrap_or("");

            let mut args_buf = [core::mem::MaybeUninit::<u8>::uninit(); 192];
            let args_len = if arg1 == 0 {
                0
            } else {
                match unsafe { copy_cstr_uninit(arg1 as *const u8, &mut args_buf) } {
                    Some(len) => len,
                    None => return u64::MAX,
                }
            };
            let args_slice =
                unsafe { slice::from_raw_parts(args_buf.as_ptr() as *const u8, args_len) };
            let args = core::str::from_utf8(args_slice).unwrap_or("");

            let mut env_buf = [core::mem::MaybeUninit::<u8>::uninit(); 512];
            let env_len = if arg2 == 0 {
                0
            } else {
                match unsafe { copy_cstr_uninit(arg2 as *const u8, &mut env_buf) } {
                    Some(len) => len,
                    None => return u64::MAX,
                }
            };
            let env_slice =
                unsafe { slice::from_raw_parts(env_buf.as_ptr() as *const u8, env_len) };
            let env = core::str::from_utf8(env_slice).unwrap_or("");

            crate::process::exec(path, args, env)
        }
        x if x == SyscallNumber::Exit as u64 => 0,
        x if x == SyscallNumber::ReadDir as u64 => {
            let path_ptr = arg0 as *const u8;
            let buf_ptr = arg1 as *mut u8;
            let buf_len = arg2 as usize;

            if path_ptr.is_null() || buf_ptr.is_null() || buf_len == 0 {
                return u64::MAX;
            }

            let mut path_buf = [core::mem::MaybeUninit::<u8>::uninit(); 128];
            let path_len = match unsafe { copy_cstr_uninit(path_ptr, &mut path_buf) } {
                Some(len) => len,
                None => return u64::MAX,
            };
            let path_slice =
                unsafe { slice::from_raw_parts(path_buf.as_ptr() as *const u8, path_len) };
            let path = core::str::from_utf8(path_slice).unwrap_or("");

            let buf = unsafe { slice::from_raw_parts_mut(buf_ptr, buf_len) };
            crate::vfs::list_dir(path, buf) as u64
        }
        x if x == SyscallNumber::ReadFile as u64 => {
            let path_ptr = arg0 as *const u8;
            let buf_ptr = arg1 as *mut u8;
            let buf_len = arg2 as usize;

            if path_ptr.is_null() || buf_ptr.is_null() || buf_len == 0 {
                return u64::MAX;
            }

            let mut path_buf = [core::mem::MaybeUninit::<u8>::uninit(); 128];
            let path_len = match unsafe { copy_cstr_uninit(path_ptr, &mut path_buf) } {
                Some(len) => len,
                None => return u64::MAX,
            };
            let path_slice =
                unsafe { slice::from_raw_parts(path_buf.as_ptr() as *const u8, path_len) };
            let path = core::str::from_utf8(path_slice).unwrap_or("");

            let buf = unsafe { slice::from_raw_parts_mut(buf_ptr, buf_len) };
            let Some(copy_len) = crate::vfs::read_file_bytes(path, buf) else {
                return u64::MAX;
            };
            copy_len as u64
        }
        x if x == SyscallNumber::ExecIo as u64 => {
            if arg0 == 0 {
                return u64::MAX;
            }

            let req = unsafe { (arg0 as *const ExecIoRequest).read() };
            if req.path_ptr.is_null() || req.status_ptr.is_null() {
                return u64::MAX;
            }

            let mut path_buf = [core::mem::MaybeUninit::<u8>::uninit(); 64];
            let path_len = match unsafe { copy_cstr_uninit(req.path_ptr, &mut path_buf) } {
                Some(len) => len,
                None => return u64::MAX,
            };
            let path_slice =
                unsafe { slice::from_raw_parts(path_buf.as_ptr() as *const u8, path_len) };
            let path = core::str::from_utf8(path_slice).unwrap_or("");

            let mut args_buf = [core::mem::MaybeUninit::<u8>::uninit(); 192];
            let args_len = if req.args_ptr.is_null() {
                0
            } else {
                match unsafe { copy_cstr_uninit(req.args_ptr, &mut args_buf) } {
                    Some(len) => len,
                    None => return u64::MAX,
                }
            };
            let args_slice =
                unsafe { slice::from_raw_parts(args_buf.as_ptr() as *const u8, args_len) };
            let args = core::str::from_utf8(args_slice).unwrap_or("");

            let mut env_buf = [core::mem::MaybeUninit::<u8>::uninit(); 512];
            let env_len = if req.env_ptr.is_null() {
                0
            } else {
                match unsafe { copy_cstr_uninit(req.env_ptr, &mut env_buf) } {
                    Some(len) => len,
                    None => return u64::MAX,
                }
            };
            let env_slice =
                unsafe { slice::from_raw_parts(env_buf.as_ptr() as *const u8, env_len) };
            let env = core::str::from_utf8(env_slice).unwrap_or("");

            let stdin = if req.stdin_ptr.is_null() || req.stdin_len == 0 {
                None
            } else {
                Some(unsafe {
                    slice::from_raw_parts(req.stdin_ptr, core::cmp::min(req.stdin_len as usize, 4096))
                })
            };

            let stdout = if req.stdout_ptr.is_null() || req.stdout_cap == 0 {
                None
            } else {
                Some(unsafe {
                    slice::from_raw_parts_mut(
                        req.stdout_ptr,
                        core::cmp::min(req.stdout_cap as usize, 4096),
                    )
                })
            };

            let (status, out_len) = crate::process::exec_with_io(path, args, env, stdin, stdout);
            unsafe {
                req.status_ptr.write(status);
            }
            out_len as u64
        }
        x if x == SyscallNumber::WriteFile as u64 => {
            let path_ptr = arg0 as *const u8;
            let buf_ptr = arg1 as *const u8;
            let buf_len = arg2 as usize;

            if path_ptr.is_null() || buf_ptr.is_null() {
                return u64::MAX;
            }

            let mut path_buf = [core::mem::MaybeUninit::<u8>::uninit(); 128];
            let path_len = match unsafe { copy_cstr_uninit(path_ptr, &mut path_buf) } {
                Some(len) => len,
                None => return u64::MAX,
            };
            let path_slice =
                unsafe { slice::from_raw_parts(path_buf.as_ptr() as *const u8, path_len) };
            let path = core::str::from_utf8(path_slice).unwrap_or("");
            let data = unsafe { slice::from_raw_parts(buf_ptr, buf_len) };
            if crate::vfs::write_file(path, data) {
                buf_len as u64
            } else {
                u64::MAX
            }
        }
        _ => u64::MAX,
    }
}

unsafe fn copy_cstr_uninit(
    src: *const u8,
    dst: &mut [core::mem::MaybeUninit<u8>],
) -> Option<usize> {
    if src.is_null() || dst.is_empty() {
        return None;
    }
    let mut i = 0usize;
    while i < dst.len() {
        let b = src.add(i).read();
        if b == 0 {
            return Some(i);
        }
        dst[i].write(b);
        i += 1;
    }
    None
}
