#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[no_mangle]
pub extern "C" fn _start(_args: *const u8, syscall_gate: usize) -> u64 {
    libos::set_syscall_gate(syscall_gate);
    init::run()
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
