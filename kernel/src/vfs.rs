use crate::serial_println;

static HELLO_TXT: &[u8] = b"hello.txt from initrd/TarFS";
static INIT_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/init.elf"));
static ECHO_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/echo.elf"));
static ENV_ELF: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/env.elf"));

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
