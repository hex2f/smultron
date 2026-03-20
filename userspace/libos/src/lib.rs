#![no_std]

use core::sync::atomic::{AtomicUsize, Ordering};

type Gate = extern "C" fn(u64, u64, u64, u64) -> u64;
static SYSCALL_GATE: AtomicUsize = AtomicUsize::new(0);

pub fn set_syscall_gate(gate: usize) {
    SYSCALL_GATE.store(gate, Ordering::Release);
}

#[inline(always)]
unsafe fn syscall3(nr: u64, a0: u64, a1: u64, a2: u64) -> u64 {
    let gate_ptr = SYSCALL_GATE.load(Ordering::Acquire);
    if gate_ptr == 0 {
        return u64::MAX;
    }
    let gate: Gate = core::mem::transmute(gate_ptr);
    gate(nr, a0, a1, a2)
}

pub fn write(fd: u64, buf: *const u8, len: u64) -> u64 {
    unsafe { syscall3(1, fd, buf as u64, len) }
}

pub fn read(fd: u64, buf: *mut u8, len: u64) -> u64 {
    unsafe { syscall3(0, fd, buf as u64, len) }
}

pub fn list_dir(path: &str, buf: &mut [u8]) -> usize {
    let mut path_buf = [core::mem::MaybeUninit::<u8>::uninit(); 128];
    let path_bytes = path.as_bytes();
    if path_bytes.len() >= 128 {
        return 0;
    }

    let mut i = 0usize;
    while i < path_bytes.len() {
        path_buf[i].write(path_bytes[i]);
        i += 1;
    }
    path_buf[path_bytes.len()].write(0);

    let ret = unsafe {
        syscall3(
            78,
            path_buf.as_ptr() as u64,
            buf.as_mut_ptr() as u64,
            buf.len() as u64,
        )
    };

    if ret == u64::MAX {
        return 0;
    }
    (ret as usize).min(buf.len())
}

pub fn read_file(path: &str, buf: &mut [u8]) -> Option<usize> {
    let mut path_buf = [core::mem::MaybeUninit::<u8>::uninit(); 128];
    let path_bytes = path.as_bytes();
    if path_bytes.len() >= 128 {
        return None;
    }

    let mut i = 0usize;
    while i < path_bytes.len() {
        path_buf[i].write(path_bytes[i]);
        i += 1;
    }
    path_buf[path_bytes.len()].write(0);

    let ret = unsafe {
        syscall3(
            79,
            path_buf.as_ptr() as u64,
            buf.as_mut_ptr() as u64,
            buf.len() as u64,
        )
    };

    if ret == u64::MAX {
        return None;
    }
    Some((ret as usize).min(buf.len()))
}

pub fn exec(path: *const u8, args: *const u8) -> u64 {
    unsafe { syscall3(59, path as u64, args as u64, 0) }
}

pub fn exec_env(path: *const u8, args: *const u8, env: *const u8) -> u64 {
    unsafe { syscall3(59, path as u64, args as u64, env as u64) }
}

pub fn exec_str(path: &str, args: &str) -> u64 {
    exec_str_env(path, args, "")
}

pub fn exec_str_env(path: &str, args: &str, env: &str) -> u64 {
    const MAX_PATH: usize = 63;
    const MAX_ARGS: usize = 191;
    const MAX_ENV: usize = 511;
    let path_bytes = path.as_bytes();
    let arg_bytes = args.as_bytes();
    let env_bytes = env.as_bytes();
    if path_bytes.len() > MAX_PATH || arg_bytes.len() > MAX_ARGS || env_bytes.len() > MAX_ENV {
        return u64::MAX;
    }

    // Avoid whole-buffer zero-init here; in this environment that can be lowered
    // to SSE ops before SSE state is explicitly configured.
    let mut path_buf = [core::mem::MaybeUninit::<u8>::uninit(); MAX_PATH + 1];
    let mut args_buf = [core::mem::MaybeUninit::<u8>::uninit(); MAX_ARGS + 1];
    let mut env_buf = [core::mem::MaybeUninit::<u8>::uninit(); MAX_ENV + 1];

    let mut i = 0usize;
    while i < path_bytes.len() {
        path_buf[i].write(path_bytes[i]);
        i += 1;
    }
    path_buf[i].write(0);

    let mut j = 0usize;
    while j < arg_bytes.len() {
        args_buf[j].write(arg_bytes[j]);
        j += 1;
    }
    args_buf[j].write(0);

    let mut k = 0usize;
    while k < env_bytes.len() {
        env_buf[k].write(env_bytes[k]);
        k += 1;
    }
    env_buf[k].write(0);

    exec_env(
        path_buf.as_ptr() as *const u8,
        args_buf.as_ptr() as *const u8,
        env_buf.as_ptr() as *const u8,
    )
}

pub fn exit(code: u64) -> ! {
    let _ = unsafe { syscall3(60, code, 0, 0) };
    loop {
        core::hint::spin_loop();
    }
}

#[no_mangle]
extern "C" fn memcpy(dst: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    let mut i = 0usize;
    while i < n {
        unsafe {
            dst.add(i).write(src.add(i).read());
        }
        i += 1;
    }
    dst
}

#[no_mangle]
extern "C" fn memset(dst: *mut u8, val: i32, n: usize) -> *mut u8 {
    let byte = val as u8;
    let mut i = 0usize;
    while i < n {
        unsafe {
            dst.add(i).write(byte);
        }
        i += 1;
    }
    dst
}

#[no_mangle]
extern "C" fn memcmp(a: *const u8, b: *const u8, n: usize) -> i32 {
    let mut i = 0usize;
    while i < n {
        let av = unsafe { a.add(i).read() };
        let bv = unsafe { b.add(i).read() };
        if av != bv {
            return av as i32 - bv as i32;
        }
        i += 1;
    }
    0
}
