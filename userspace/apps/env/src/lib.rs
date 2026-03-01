#![no_std]

pub fn run(env: &str) {
    let bytes = env.as_bytes();
    if !bytes.is_empty() {
        let _ = libos::write(1, bytes.as_ptr(), bytes.len() as u64);
    }
}
