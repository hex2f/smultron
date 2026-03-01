#![no_std]

pub fn run(args: &str) {
    let bytes = args.as_bytes();
    if !bytes.is_empty() {
        let _ = libos::write(1, bytes.as_ptr(), bytes.len() as u64);
    }
    let _ = libos::write(1, b"\n".as_ptr(), 1);
}
