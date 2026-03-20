use crate::serial_println;

static HELLO_TXT: &[u8] = b"hello.txt from initrd/TarFS";
static INIT_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/init.elf"));
static ECHO_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/echo.elf"));
static ENV_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/env.elf"));
static LS_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/ls.elf"));
static CAT_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/cat.elf"));

struct FileEntry {
    path: &'static str,
    data: &'static [u8],
}

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
        if entry.path.starts_with(prefix_str) {
            let remainder = &entry.path[prefix_len..];
            let component = remainder.split('/').next().unwrap_or("");
            if !component.is_empty() {
                // Check if we already added this component
                let mut already_added = false;
                let mut check_offset = 0;
                while check_offset < offset {
                    let mut len = 0;
                    while check_offset + len < offset && buf[check_offset + len] != 0 {
                        len += 1;
                    }
                    if len == component.len()
                        && &buf[check_offset..check_offset + len] == component.as_bytes()
                    {
                        already_added = true;
                        break;
                    }
                    check_offset += len + 1;
                }

                if !already_added {
                    let comp_bytes = component.as_bytes();
                    if offset + comp_bytes.len() + 1 <= buf.len() {
                        buf[offset..offset + comp_bytes.len()].copy_from_slice(comp_bytes);
                        offset += comp_bytes.len();
                        buf[offset] = 0; // null-terminated
                        offset += 1;
                    }
                }
            }
        }
    }
    offset
}
