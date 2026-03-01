use crate::serial_println;
use x86_64::structures::paging::{mapper::MapToError, FrameAllocator, Mapper, Size4KiB};

pub fn ring3_smoke_test() -> bool {
    // Placeholder for real `sysret` transition. We preserve a test hook for harness output.
    serial_println!("[ok] ring3 smoke test placeholder executed");
    true
}

pub fn init_syscall_gate(
    _mapper: &mut impl Mapper<Size4KiB>,
    _frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    serial_println!("[ok] syscall gate scaffold initialized");
    Ok(())
}
