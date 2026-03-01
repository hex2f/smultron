use crate::{elf_loader, serial_println, vfs};
use spin::Mutex;
use x86_64::{
    structures::paging::{
        mapper::MapToError, FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB,
    },
    VirtAddr,
};

pub const PROCESS_SLOT_BASE: u64 = 0x0000_5555_0000_0000;
pub const PROCESS_SLOT_SIZE: u64 = 0x0010_0000;
pub const MAX_PROCESS_SLOTS: usize = 4;
const MAX_PROCESSES: usize = 16;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProcessState {
    Empty,
    Running,
}

#[derive(Clone, Copy)]
struct ProcessRecord {
    pid: u64,
    slot_idx: usize,
    state: ProcessState,
    exit_code: u64,
}

impl ProcessRecord {
    const fn empty() -> Self {
        Self {
            pid: 0,
            slot_idx: 0,
            state: ProcessState::Empty,
            exit_code: 0,
        }
    }
}

struct ProcessManager {
    next_pid: u64,
    slots_in_use: [bool; MAX_PROCESS_SLOTS],
    records: [ProcessRecord; MAX_PROCESSES],
}

impl ProcessManager {
    const fn new() -> Self {
        Self {
            next_pid: 1,
            slots_in_use: [false; MAX_PROCESS_SLOTS],
            records: [ProcessRecord::empty(); MAX_PROCESSES],
        }
    }

    fn spawn_slot(&mut self) -> Option<(u64, usize)> {
        let mut slot_idx = 0usize;
        while slot_idx < MAX_PROCESS_SLOTS {
            if !self.slots_in_use[slot_idx] {
                break;
            }
            slot_idx += 1;
        }
        if slot_idx == MAX_PROCESS_SLOTS {
            return None;
        }

        let mut rec_idx = 0usize;
        while rec_idx < MAX_PROCESSES {
            if self.records[rec_idx].state == ProcessState::Empty {
                break;
            }
            rec_idx += 1;
        }
        if rec_idx == MAX_PROCESSES {
            return None;
        }

        let pid = self.next_pid;
        self.next_pid = self.next_pid.saturating_add(1);
        self.slots_in_use[slot_idx] = true;
        self.records[rec_idx] = ProcessRecord {
            pid,
            slot_idx,
            state: ProcessState::Running,
            exit_code: 0,
        };
        Some((pid, slot_idx))
    }

    fn finish(&mut self, pid: u64, exit_code: u64) {
        let mut i = 0usize;
        while i < MAX_PROCESSES {
            if self.records[i].pid == pid && self.records[i].state == ProcessState::Running {
                let slot_idx = self.records[i].slot_idx;
                self.records[i].exit_code = exit_code;
                if slot_idx < MAX_PROCESS_SLOTS {
                    self.slots_in_use[slot_idx] = false;
                }
                self.records[i] = ProcessRecord::empty();
                return;
            }
            i += 1;
        }
    }
}

static PROCESS_MANAGER: Mutex<ProcessManager> = Mutex::new(ProcessManager::new());

pub fn init_exec_regions(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    let mut slot = 0usize;
    while slot < MAX_PROCESS_SLOTS {
        let base = slot_base(slot);
        map_range(mapper, frame_allocator, base, PROCESS_SLOT_SIZE)?;
        slot += 1;
    }
    serial_println!("[ok] process exec regions mapped");
    Ok(())
}

pub fn probe_executable(path: &str) -> bool {
    let Some(bytes) = vfs::read_file(path) else {
        return false;
    };
    let slot = slot_for_index(0);
    elf_loader::probe_elf_for_slot(bytes, slot)
}

pub fn exec(path: &str, args: &str) -> u64 {
    let Some(bytes) = vfs::read_file(path) else {
        serial_println!("[failed] exec: missing app '{}'", path);
        return u64::MAX;
    };

    let (pid, slot_idx) = {
        let mut pm = PROCESS_MANAGER.lock();
        match pm.spawn_slot() {
            Some(v) => v,
            None => {
                serial_println!("[failed] exec: no free process slot");
                return u64::MAX;
            }
        }
    };

    let slot = slot_for_index(slot_idx);
    let status = match elf_loader::exec_in_slot(bytes, slot, args) {
        Ok(code) => code,
        Err(msg) => {
            serial_println!("[failed] exec: {} for '{}'", msg, path);
            u64::MAX
        }
    };

    PROCESS_MANAGER.lock().finish(pid, status);
    status
}

fn slot_for_index(slot_idx: usize) -> elf_loader::AppSlot {
    let base = slot_base(slot_idx);
    elf_loader::AppSlot {
        base,
        end: base + PROCESS_SLOT_SIZE,
    }
}

fn slot_base(slot_idx: usize) -> u64 {
    PROCESS_SLOT_BASE + (slot_idx as u64) * PROCESS_SLOT_SIZE
}

fn map_range(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    base: u64,
    size: u64,
) -> Result<(), MapToError<Size4KiB>> {
    let start = VirtAddr::new(base);
    let end = VirtAddr::new(base + size - 1);
    let start_page = Page::containing_address(start);
    let end_page = Page::containing_address(end);
    let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

    for page in Page::range_inclusive(start_page, end_page) {
        if mapper.translate_page(page).is_ok() {
            continue;
        }
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        unsafe {
            mapper.map_to(page, frame, flags, frame_allocator)?.flush();
        }
    }

    Ok(())
}
