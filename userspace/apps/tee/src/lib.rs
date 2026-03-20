#![no_std]

const MAX_ARGS: usize = 16;
const MAX_STREAM: usize = 4096;

pub fn run(args: &str) {
    let mut files = [""; MAX_ARGS];
    let mut file_count = 0usize;

    for tok in args.split_whitespace() {
        if file_count >= MAX_ARGS {
            write_str("tee: too many output files\n");
            return;
        }
        files[file_count] = tok;
        file_count += 1;
    }

    let mut stream = [0u8; MAX_STREAM];
    let mut stream_len = 0usize;
    let mut chunk = [0u8; 256];

    loop {
        let n = libos::read(0, chunk.as_mut_ptr(), chunk.len() as u64);
        if n == u64::MAX {
            write_str("tee: read error\n");
            return;
        }
        if n == 0 {
            break;
        }

        let n = n as usize;
        let _ = libos::write(1, chunk.as_ptr(), n as u64);

        if stream_len < stream.len() {
            let copy_len = core::cmp::min(n, stream.len() - stream_len);
            stream[stream_len..stream_len + copy_len].copy_from_slice(&chunk[..copy_len]);
            stream_len += copy_len;
        }
    }

    let mut i = 0usize;
    while i < file_count {
        if libos::write_file(files[i], &stream[..stream_len]).is_none() {
            write_str("tee: cannot write ");
            write_str(files[i]);
            write_str("\n");
        }
        i += 1;
    }
}

fn write_str(s: &str) {
    let _ = libos::write(1, s.as_ptr(), s.len() as u64);
}
