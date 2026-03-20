use crate::serial_println;
use spin::Mutex;

static HELLO_TXT: &[u8] = b"hello.txt from initrd/TarFS";
static INIT_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/init.elf"));
static ECHO_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/echo.elf"));
static ENV_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/env.elf"));
static LS_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/ls.elf"));
static CAT_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/cat.elf"));
static TEE_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/tee.elf"));

struct FileEntry {
    path: &'static str,
    data: &'static [u8],
}

const MAX_RAM_FILES: usize = 16;
const MAX_RAM_PATH_LEN: usize = 128;
const MAX_RAM_FILE_LEN: usize = 4096;

#[derive(Clone, Copy)]
struct RamFile {
    used: bool,
    path_len: usize,
    data_len: usize,
    path: [u8; MAX_RAM_PATH_LEN],
    data: [u8; MAX_RAM_FILE_LEN],
}

impl RamFile {
    const fn empty() -> Self {
        Self {
            used: false,
            path_len: 0,
            data_len: 0,
            path: [0; MAX_RAM_PATH_LEN],
            data: [0; MAX_RAM_FILE_LEN],
        }
    }
}

static RAM_FILES: Mutex<[RamFile; MAX_RAM_FILES]> = Mutex::new([RamFile::empty(); MAX_RAM_FILES]);

static FILES: &[FileEntry] = &[
    FileEntry {
        path: "/hello.txt",
        data: HELLO_TXT,
    },
    FileEntry {
        path: "/bin/init",
        data: INIT_ELF,
    },
    FileEntry {
        path: "/bin/echo",
        data: ECHO_ELF,
    },
    FileEntry {
        path: "/bin/env",
        data: ENV_ELF,
    },
    FileEntry {
        path: "/bin/ls",
        data: LS_ELF,
    },
    FileEntry {
        path: "/bin/cat",
        data: CAT_ELF,
    },
    FileEntry {
        path: "/bin/tee",
        data: TEE_ELF,
    },
];

pub fn init() {
    serial_println!("[ok] vfs initialized");
}

pub fn read_file(path: &str) -> Option<&'static [u8]> {
    for entry in FILES {
        if entry.path == path {
            return Some(entry.data);
        }
    }
    None
}

pub fn read_file_bytes(path: &str, out: &mut [u8]) -> Option<usize> {
    if let Some(data) = read_file(path) {
        let copy_len = core::cmp::min(data.len(), out.len());
        out[..copy_len].copy_from_slice(&data[..copy_len]);
        return Some(copy_len);
    }

    let ram = RAM_FILES.lock();
    for file in ram.iter() {
        if !file.used {
            continue;
        }
        let path_bytes = path.as_bytes();
        if file.path_len != path_bytes.len() || &file.path[..file.path_len] != path_bytes {
            continue;
        }

        let copy_len = core::cmp::min(file.data_len, out.len());
        out[..copy_len].copy_from_slice(&file.data[..copy_len]);
        return Some(copy_len);
    }
    None
}

pub fn write_file(path: &str, data: &[u8]) -> bool {
    let path_bytes = path.as_bytes();
    if path_bytes.is_empty() || path_bytes.len() > MAX_RAM_PATH_LEN {
        return false;
    }

    let copy_len = core::cmp::min(data.len(), MAX_RAM_FILE_LEN);
    let mut ram = RAM_FILES.lock();

    for file in ram.iter_mut() {
        if file.used
            && file.path_len == path_bytes.len()
            && &file.path[..file.path_len] == path_bytes
        {
            file.data_len = copy_len;
            file.data[..copy_len].copy_from_slice(&data[..copy_len]);
            return true;
        }
    }

    for file in ram.iter_mut() {
        if !file.used {
            file.used = true;
            file.path_len = path_bytes.len();
            file.path[..path_bytes.len()].copy_from_slice(path_bytes);
            file.data_len = copy_len;
            file.data[..copy_len].copy_from_slice(&data[..copy_len]);
            return true;
        }
    }
    false
}

pub fn list_dir(path: &str, buf: &mut [u8]) -> usize {
    let mut offset = 0;

    // Normalize dir path to have a trailing slash
    let mut dir_prefix = [core::mem::MaybeUninit::<u8>::uninit(); 128];
    let path_bytes = path.as_bytes();
    let prefix_len = if path == "/" {
        dir_prefix[0].write(b'/');
        1
    } else {
        let len = core::cmp::min(path_bytes.len(), 126);
        let mut i = 0usize;
        while i < len {
            dir_prefix[i].write(path_bytes[i]);
            i += 1;
        }
        dir_prefix[len].write(b'/');
        len + 1
    };

    let prefix_bytes =
        unsafe { core::slice::from_raw_parts(dir_prefix.as_ptr() as *const u8, prefix_len) };
    let prefix_str = core::str::from_utf8(prefix_bytes).unwrap_or("/");

    for entry in FILES {
        append_path_component(entry.path, prefix_str, prefix_len, buf, &mut offset);
    }
    let ram = RAM_FILES.lock();
    for file in ram.iter() {
        if !file.used {
            continue;
        }
        let path = core::str::from_utf8(&file.path[..file.path_len]).unwrap_or("");
        append_path_component(path, prefix_str, prefix_len, buf, &mut offset);
    }
    offset
}

fn append_path_component(
    entry_path: &str,
    prefix_str: &str,
    prefix_len: usize,
    buf: &mut [u8],
    offset: &mut usize,
) {
    if !entry_path.starts_with(prefix_str) {
        return;
    }
    let remainder = &entry_path[prefix_len..];
    let component = remainder.split('/').next().unwrap_or("");
    if component.is_empty() {
        return;
    }

    let mut already_added = false;
    let mut check_offset = 0;
    while check_offset < *offset {
        let mut len = 0;
        while check_offset + len < *offset && buf[check_offset + len] != 0 {
            len += 1;
        }
        if len == component.len() && &buf[check_offset..check_offset + len] == component.as_bytes()
        {
            already_added = true;
            break;
        }
        check_offset += len + 1;
    }

    if !already_added {
        let comp_bytes = component.as_bytes();
        if *offset + comp_bytes.len() + 1 <= buf.len() {
            buf[*offset..*offset + comp_bytes.len()].copy_from_slice(comp_bytes);
            *offset += comp_bytes.len();
            buf[*offset] = 0;
            *offset += 1;
        }
    }
}
