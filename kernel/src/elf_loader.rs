use crate::syscall;

const MAX_ARG_LEN: usize = 191;
const MAX_ENV_LEN: usize = 511;
const ELFMAG: &[u8; 4] = b"\x7fELF";
const EI_CLASS: usize = 4;
const EI_DATA: usize = 5;
const ELFCLASS64: u8 = 2;
const ELFDATA2LSB: u8 = 1;
const PT_LOAD: u32 = 1;
const ELF64_EHDR_SIZE: usize = 64;
const ELF64_PHDR_SIZE: usize = 56;

type EntryFn = extern "C" fn(*const u8, usize, *const u8) -> u64;

#[derive(Clone, Copy)]
pub struct AppSlot {
    pub base: u64,
    pub end: u64,
}

pub fn probe_elf_for_slot(bytes: &[u8], slot: AppSlot) -> bool {
    validate_elf_for_slot(bytes, slot).is_ok()
}

pub fn exec_in_slot(
    bytes: &[u8],
    slot: AppSlot,
    args: &str,
    env: &str,
) -> Result<u64, &'static str> {
    let hdr = match validate_elf_for_slot(bytes, slot) {
        Ok(v) => v,
        Err(msg) => {
            return Err(msg);
        }
    };

    if load_segments(bytes, slot, &hdr).is_err() {
        return Err("PT_LOAD mapping failed");
    }

    if hdr.entry == 0 {
        return Err("invalid ELF entry");
    }

    if args.len() > MAX_ARG_LEN {
        return Err("args too long");
    }

    if env.len() > MAX_ENV_LEN {
        return Err("env too long");
    }

    // Avoid whole-buffer zero-init on stack here; it can lower to SSE ops before
    // we've explicitly enabled SSE in this kernel path.
    let mut arg_buf = [core::mem::MaybeUninit::<u8>::uninit(); MAX_ARG_LEN + 1];
    let mut i = 0usize;
    while i < args.len() {
        arg_buf[i].write(args.as_bytes()[i]);
        i += 1;
    }
    arg_buf[i].write(0);

    let mut env_buf = [core::mem::MaybeUninit::<u8>::uninit(); MAX_ENV_LEN + 1];
    let mut j = 0usize;
    while j < env.len() {
        env_buf[j].write(env.as_bytes()[j]);
        j += 1;
    }
    env_buf[j].write(0);

    let entry_fn: EntryFn = unsafe { core::mem::transmute(hdr.entry as usize) };
    let gate = syscall::smultron_syscall_gate as *const () as usize;
    Ok(entry_fn(
        arg_buf.as_ptr() as *const u8,
        gate,
        env_buf.as_ptr() as *const u8,
    ))
}

fn validate_elf_for_slot(bytes: &[u8], slot: AppSlot) -> Result<ElfHdr, &'static str> {
    let hdr = parse_elf_header(bytes)?;

    for ph in program_headers(bytes, &hdr)? {
        if ph.p_type != PT_LOAD {
            continue;
        }

        let mem_end = ph
            .p_vaddr
            .checked_add(ph.p_memsz)
            .ok_or("PT_LOAD range overflow")?;
        if ph.p_vaddr < slot.base || mem_end > slot.end {
            return Err("PT_LOAD outside app slot");
        }

        let file_end = ph
            .p_offset
            .checked_add(ph.p_filesz)
            .ok_or("PT_LOAD file range overflow")?;
        if file_end as usize > bytes.len() {
            return Err("PT_LOAD file range outside ELF");
        }

        if ph.p_memsz < ph.p_filesz {
            return Err("PT_LOAD memsz < filesz");
        }
    }

    Ok(hdr)
}

fn load_segments(bytes: &[u8], slot: AppSlot, hdr: &ElfHdr) -> Result<(), ()> {
    unsafe {
        core::ptr::write_bytes(slot.base as *mut u8, 0, (slot.end - slot.base) as usize);
    }

    for ph in program_headers(bytes, hdr).map_err(|_| ())? {
        if ph.p_type != PT_LOAD {
            continue;
        }

        let file_size = ph.p_filesz as usize;
        let mem_size = ph.p_memsz as usize;
        let offset = ph.p_offset as usize;
        let dst = ph.p_vaddr as *mut u8;

        unsafe {
            core::ptr::copy_nonoverlapping(bytes.as_ptr().add(offset), dst, file_size);
            if mem_size > file_size {
                core::ptr::write_bytes(dst.add(file_size), 0, mem_size - file_size);
            }
        }
    }

    Ok(())
}

fn parse_elf_header(bytes: &[u8]) -> Result<ElfHdr, &'static str> {
    if bytes.len() < ELF64_EHDR_SIZE {
        return Err("ELF too small");
    }
    if &bytes[0..4] != ELFMAG {
        return Err("invalid ELF magic");
    }
    if bytes[EI_CLASS] != ELFCLASS64 {
        return Err("unsupported ELF class");
    }
    if bytes[EI_DATA] != ELFDATA2LSB {
        return Err("unsupported ELF endianness");
    }

    let entry = read_u64(bytes, 24)?;
    let phoff = read_u64(bytes, 32)?;
    let phentsize = read_u16(bytes, 54)?;
    let phnum = read_u16(bytes, 56)?;

    if phentsize as usize != ELF64_PHDR_SIZE {
        return Err("unexpected program header size");
    }

    Ok(ElfHdr {
        entry,
        phoff,
        phentsize,
        phnum,
    })
}

fn program_headers<'a>(
    bytes: &'a [u8],
    hdr: &'a ElfHdr,
) -> Result<ProgramHeaderIter<'a>, &'static str> {
    let total = (hdr.phentsize as u64)
        .checked_mul(hdr.phnum as u64)
        .ok_or("program header overflow")?;
    let end = hdr
        .phoff
        .checked_add(total)
        .ok_or("program header overflow")?;
    if end as usize > bytes.len() {
        return Err("program header table out of bounds");
    }

    Ok(ProgramHeaderIter { bytes, hdr, idx: 0 })
}

fn read_u16(bytes: &[u8], off: usize) -> Result<u16, &'static str> {
    let end = off.checked_add(2).ok_or("offset overflow")?;
    let slice = bytes.get(off..end).ok_or("offset out of bounds")?;
    Ok(u16::from_le_bytes([slice[0], slice[1]]))
}

fn read_u32(bytes: &[u8], off: usize) -> Result<u32, &'static str> {
    let end = off.checked_add(4).ok_or("offset overflow")?;
    let slice = bytes.get(off..end).ok_or("offset out of bounds")?;
    Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn read_u64(bytes: &[u8], off: usize) -> Result<u64, &'static str> {
    let end = off.checked_add(8).ok_or("offset overflow")?;
    let slice = bytes.get(off..end).ok_or("offset out of bounds")?;
    Ok(u64::from_le_bytes([
        slice[0], slice[1], slice[2], slice[3], slice[4], slice[5], slice[6], slice[7],
    ]))
}

struct ElfHdr {
    entry: u64,
    phoff: u64,
    phentsize: u16,
    phnum: u16,
}

#[derive(Clone, Copy)]
struct ProgramHeader {
    p_type: u32,
    p_offset: u64,
    p_vaddr: u64,
    p_filesz: u64,
    p_memsz: u64,
}

struct ProgramHeaderIter<'a> {
    bytes: &'a [u8],
    hdr: &'a ElfHdr,
    idx: u16,
}

impl<'a> Iterator for ProgramHeaderIter<'a> {
    type Item = ProgramHeader;

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx >= self.hdr.phnum {
            return None;
        }

        let base = self.hdr.phoff as usize + self.idx as usize * self.hdr.phentsize as usize;
        self.idx += 1;

        let p_type = read_u32(self.bytes, base).ok()?;
        let p_offset = read_u64(self.bytes, base + 8).ok()?;
        let p_vaddr = read_u64(self.bytes, base + 16).ok()?;
        let p_filesz = read_u64(self.bytes, base + 32).ok()?;
        let p_memsz = read_u64(self.bytes, base + 40).ok()?;

        Some(ProgramHeader {
            p_type,
            p_offset,
            p_vaddr,
            p_filesz,
            p_memsz,
        })
    }
}
