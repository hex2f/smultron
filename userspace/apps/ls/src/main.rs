#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[no_mangle]
pub extern "C" fn _start(_args: *const u8, syscall_gate: usize, env: *const u8) -> u64 {
    libos::set_syscall_gate(syscall_gate);
    let env_str = unsafe { cstr_to_str(env) };

    // Simple way to get CWD: parse env_str
    let mut cwd = "/";
    for line in env_str.split('\n') {
        if let Some((k, v)) = line.split_once('=') {
            if k == "CWD" {
                cwd = v;
                break;
            }
        }
    }

    ls::run(cwd);
    0
}

unsafe fn cstr_to_str(ptr: *const u8) -> &'static str {
    if ptr.is_null() {
        return "";
    }
    let mut len = 0usize;
    while ptr.add(len).read() != 0 {
        len += 1;
    }
    let bytes = core::slice::from_raw_parts(ptr, len);
    core::str::from_utf8(bytes).unwrap_or("")
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
