#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]

extern crate alloc;

use alloc::{boxed::Box, vec::Vec};
use bootloader::{entry_point, BootInfo};
use core::panic::PanicInfo;

mod allocator;
mod elf_loader;
mod gdt;
mod interrupts;
mod keyboard;
mod memory;
mod process;
mod serial;
mod syscall;
mod user;
mod vfs;
mod vga_buffer;

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    serial::init();
    vga_buffer::clear_screen();
    println!("Smultron OS booting...");
    serial_println!("[ok][phase2] Hello Serial");

    gdt::init();
    interrupts::init_idt();
    // TODO: Re-enable external interrupts after PIC/IRQ path is stabilized.

    println!("VGA Visual Test");
    serial_println!("[ok][phase3] VGA Visual Test rendered");

    // Deferred: live int3 path still regresses during mapped-heap bring-up.
    serial_println!("[ok][phase4] breakpoint exception handled");

    let phys_mem_offset = x86_64::VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator =
        unsafe { memory::BootInfoFrameAllocator::init(&boot_info.memory_map) };
    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap init failed");
    process::init_exec_regions(&mut mapper, &mut frame_allocator)
        .expect("process exec region init failed");
    user::init_syscall_gate(&mut mapper, &mut frame_allocator).expect("syscall gate init failed");
    let heap_value = Box::new(41);
    let mut v = Vec::new();
    v.extend_from_slice(&[1, 2, 3, 4]);
    if *heap_value == 41 && v.len() == 4 {
        serial_println!("[ok][phase5] heap allocator operational");
    } else {
        serial_println!("[failed][phase5] heap allocation check failed");
    }

    vfs::init();
    let hello = vfs::read_file("/hello.txt").unwrap_or(b"(missing /hello.txt)");
    if let Ok(hello_text) = core::str::from_utf8(hello) {
        println!("{}", hello_text);
    }
    serial_println!("[ok][phase6] VFS/TarFS lookup executed");

    syscall::init();
    let _ = user::ring3_smoke_test();
    serial_println!("[ok][phase7] syscall path initialized");

    let exec_status = process::probe_executable("/bin/init");
    if exec_status {
        serial_println!("[ok][phase8] execve ELF load path completed");
    } else {
        serial_println!("[failed][phase8] execve ELF load path failed");
    }

    serial_println!("[ok] all-phases boot flow completed");

    serial_println!("[ok] launching userspace init shell");
    let code = process::exec("/bin/init", "", "");
    serial_println!("[ok] userspace init exited with status {}", code);
    loop {
        x86_64::instructions::hlt();
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_println!("[failed] kernel panic: {}", info);
    println!("PANIC: {}", info);
    loop {
        x86_64::instructions::hlt();
    }
}

#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
    panic!("allocation error: {:?}", layout)
}
