#![no_std]

use core::str;

const MAX_LINE: usize = 256;
const BIN_PREFIX: &str = "/bin/";
const MAX_ENV_VARS: usize = 16;
const MAX_ENV_LEN: usize = 511;
const MAX_PIPE_STAGES: usize = 4;
const MAX_STAGE_TOKENS: usize = 32;
const MAX_ARGS: usize = 191;
const MAX_IO_BUF: usize = 4096;
const MAX_PARSED_LINE: usize = MAX_LINE * 2;
const MAX_TOKENS: usize = 64;

#[derive(Clone, Copy)]
struct EnvVar {
    key: [u8; 32],
    key_len: usize,
    val: [u8; 64],
    val_len: usize,
}

impl EnvVar {
    const fn empty() -> Self {
        Self {
            key: [0; 32],
            key_len: 0,
            val: [0; 64],
            val_len: 0,
        }
    }
}

static mut ENV: [EnvVar; MAX_ENV_VARS] = [EnvVar::empty(); MAX_ENV_VARS];

struct StageParse<'a> {
    cmd: &'a str,
    args_len: usize,
    in_file: Option<&'a str>,
    out_file: Option<&'a str>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TokenKind {
    Word,
    Pipe,
    RedirectIn,
    RedirectOut,
}

#[derive(Clone, Copy)]
struct Token {
    kind: TokenKind,
    start: usize,
    len: usize,
}

pub fn run() -> u64 {
    set_env("CWD", "/");

    write_str("smultron shell (userspace)\n");
    write_str("type 'help' for commands\n");

    let mut line = [0u8; MAX_LINE];
    loop {
        write_str("smultron$ ");
        let len = read_line(&mut line);
        if !dispatch(&line[..len]) {
            return 0;
        }
    }
}

fn write_str(s: &str) {
    let _ = libos::write(1, s.as_ptr(), s.len() as u64);
}

fn read_line(buf: &mut [u8; MAX_LINE]) -> usize {
    let mut len = 0usize;
    loop {
        let mut byte = 0u8;
        if libos::read(0, &mut byte as *mut u8, 1) != 1 {
            continue;
        }

        match byte {
            b'\r' | b'\n' => {
                write_str("\n");
                return len;
            }
            8 | 127 => {
                if len > 0 {
                    len -= 1;
                    write_str("\x08 \x08");
                }
            }
            b => {
                if len < MAX_LINE {
                    buf[len] = b;
                    len += 1;
                    let _ = libos::write(1, &b as *const u8, 1);
                }
            }
        }
    }
}

fn set_env(key: &str, val: &str) {
    if key.len() > 32 || val.len() > 64 {
        write_str("error: env var too long\n");
        return;
    }

    let key_bytes = key.as_bytes();
    let val_bytes = val.as_bytes();

    unsafe {
        let mut empty_idx = None;
        for i in 0..MAX_ENV_VARS {
            if ENV[i].key_len == key.len() && &ENV[i].key[..key.len()] == key_bytes {
                ENV[i].val_len = val.len();
                ENV[i].val[..val.len()].copy_from_slice(val_bytes);
                return;
            }
            if ENV[i].key_len == 0 && empty_idx.is_none() {
                empty_idx = Some(i);
            }
        }

        if let Some(i) = empty_idx {
            ENV[i].key_len = key.len();
            ENV[i].key[..key.len()].copy_from_slice(key_bytes);
            ENV[i].val_len = val.len();
            ENV[i].val[..val.len()].copy_from_slice(val_bytes);
        } else {
            write_str("error: too many env vars\n");
        }
    }
}

fn build_env_str() -> ([u8; MAX_ENV_LEN + 1], usize) {
    let mut buf = [0u8; MAX_ENV_LEN + 1];
    let mut offset = 0;

    unsafe {
        for i in 0..MAX_ENV_VARS {
            if ENV[i].key_len > 0 {
                let pair_len = ENV[i].key_len + 1 + ENV[i].val_len + 1;
                if offset + pair_len > MAX_ENV_LEN {
                    break;
                }

                buf[offset..offset + ENV[i].key_len]
                    .copy_from_slice(&ENV[i].key[..ENV[i].key_len]);
                offset += ENV[i].key_len;

                buf[offset] = b'=';
                offset += 1;

                buf[offset..offset + ENV[i].val_len]
                    .copy_from_slice(&ENV[i].val[..ENV[i].val_len]);
                offset += ENV[i].val_len;

                buf[offset] = b'\n';
                offset += 1;
            }
        }
    }
    (buf, offset)
}

fn get_env(key: &str) -> Option<&[u8]> {
    let key_bytes = key.as_bytes();
    unsafe {
        for i in 0..MAX_ENV_VARS {
            if ENV[i].key_len == key.len() && &ENV[i].key[..key.len()] == key_bytes {
                return Some(&ENV[i].val[..ENV[i].val_len]);
            }
        }
    }
    None
}

fn expand_line(line: &str, out: &mut [u8]) -> usize {
    let bytes = line.as_bytes();
    let mut i = 0;
    let mut out_idx = 0;

    while i < bytes.len() && out_idx < out.len() {
        if bytes[i] == b'$' {
            i += 1;
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            if start == i {
                if out_idx < out.len() {
                    out[out_idx] = b'$';
                    out_idx += 1;
                }
            } else {
                let var_name = core::str::from_utf8(&bytes[start..i]).unwrap_or("");
                if let Some(v) = get_env(var_name) {
                    for &b in v {
                        if out_idx < out.len() {
                            out[out_idx] = b;
                            out_idx += 1;
                        }
                    }
                }
            }
        } else {
            if out_idx < out.len() {
                out[out_idx] = bytes[i];
                out_idx += 1;
            }
            i += 1;
        }
    }
    out_idx
}

fn dispatch(line: &[u8]) -> bool {
    let command = str::from_utf8(line).unwrap_or("").trim();
    if command.is_empty() {
        return true;
    }

    let mut expanded_buf = [0u8; MAX_LINE * 2];
    let expanded_len = expand_line(command, &mut expanded_buf);
    let expanded_str = core::str::from_utf8(&expanded_buf[..expanded_len])
        .unwrap_or("")
        .trim();

    if expanded_str.is_empty() {
        return true;
    }

    let mut parsed_storage = [0u8; MAX_PARSED_LINE];
    let mut tokens = [Token {
        kind: TokenKind::Word,
        start: 0,
        len: 0,
    }; MAX_TOKENS];
    let token_count = match tokenize_command(expanded_str, &mut parsed_storage, &mut tokens) {
        Ok(n) => n,
        Err(msg) => {
            write_str("parse error: ");
            write_str(msg);
            write_str("\n");
            return true;
        }
    };
    if token_count == 0 {
        return true;
    }

    let mut has_ops = false;
    let mut i = 0usize;
    while i < token_count {
        if tokens[i].kind != TokenKind::Word {
            has_ops = true;
            break;
        }
        i += 1;
    }

    if has_ops {
        dispatch_compound_tokens(&tokens[..token_count], &parsed_storage)
    } else {
        let cmd = token_word(tokens[0], &parsed_storage);
        let mut args_buf = [0u8; MAX_ARGS];
        let args_len = match join_word_tokens(&tokens[1..token_count], &parsed_storage, &mut args_buf) {
            Some(n) => n,
            None => {
                write_str("parse error: args too long\n");
                return true;
            }
        };
        let args = core::str::from_utf8(&args_buf[..args_len]).unwrap_or("");
        dispatch_simple(cmd, args)
    }
}

fn dispatch_simple(cmd: &str, args: &str) -> bool {
    match cmd {
        "help" => {
            write_str("commands: help, clear, exit, env, export, cd, <binary in /bin>\n");
            write_str("shell I/O: cmd1 | cmd2, cmd > file, cmd < file\n");
            write_str("quotes: use '...' or \"...\" for grouped args/paths\n");
            true
        }
        "clear" => {
            write_str("\x1b[2J\x1b[H");
            true
        }
        "exit" => false,
        "cd" => {
            set_cwd(args);
            true
        }
        "env" => {
            let (env_buf, env_len) = build_env_str();
            let env_str = core::str::from_utf8(&env_buf[..env_len]).unwrap_or("");
            write_str(env_str);
            true
        }
        "export" => {
            if let Some((k, v)) = args.split_once('=') {
                set_env(k.trim(), v.trim());
            } else {
                write_str("usage: export KEY=VALUE\n");
            }
            true
        }
        _ => {
            if exec_from_bin(cmd, args) == u64::MAX {
                write_str("unknown command: ");
                write_str(cmd);
                write_str("\n");
            }
            true
        }
    }
}

fn dispatch_compound_tokens(tokens: &[Token], parsed: &[u8]) -> bool {
    let mut pipe_data = [0u8; MAX_IO_BUF];
    let mut pipe_len = 0usize;
    let mut capture_buf = [0u8; MAX_IO_BUF];
    let mut stage_starts = [0usize; MAX_PIPE_STAGES];
    let mut stage_ends = [0usize; MAX_PIPE_STAGES];
    let mut stage_count = 0usize;
    let mut start = 0usize;
    let mut i = 0usize;
    while i <= tokens.len() {
        let is_end = i == tokens.len();
        let is_pipe = !is_end && tokens[i].kind == TokenKind::Pipe;
        if is_end || is_pipe {
            if stage_count >= MAX_PIPE_STAGES {
                write_str("parse error: too many pipeline stages\n");
                return true;
            }
            if start == i {
                write_str("parse error: empty pipeline stage\n");
                return true;
            }
            stage_starts[stage_count] = start;
            stage_ends[stage_count] = i;
            stage_count += 1;
            start = i + 1;
        }
        i += 1;
    }

    let mut idx = 0usize;
    while idx < stage_count {
        let mut args_buf = [0u8; MAX_ARGS];
        let stage = match parse_stage(
            &tokens[stage_starts[idx]..stage_ends[idx]],
            parsed,
            idx == 0,
            idx + 1 == stage_count,
            &mut args_buf,
        ) {
            Some(s) => s,
            None => return true,
        };

        if is_builtin(stage.cmd) {
            write_str("error: pipes/redirection are only supported for /bin commands\n");
            return true;
        }

        if idx == 0 {
            if let Some(path) = stage.in_file {
                let mut file_buf = [0u8; MAX_IO_BUF];
                let mut path_storage = [core::mem::MaybeUninit::<u8>::uninit(); 128];
                let file_path = match resolve_path(path, &mut path_storage) {
                    Some(p) => p,
                    None => {
                        write_str("redirection error: path too long\n");
                        return true;
                    }
                };
                match libos::read_file(file_path, &mut file_buf) {
                    Some(len) => {
                        pipe_len = len;
                        pipe_data[..len].copy_from_slice(&file_buf[..len]);
                    }
                    None => {
                        write_str("redirection error: cannot read ");
                        write_str(file_path);
                        write_str("\n");
                        return true;
                    }
                }
            }
        }

        let args = core::str::from_utf8(&args_buf[..stage.args_len]).unwrap_or("");
        let stdin = if idx == 0 {
            if stage.in_file.is_some() {
                Some(&pipe_data[..pipe_len])
            } else {
                None
            }
        } else {
            Some(&pipe_data[..pipe_len])
        };

        let capture = idx + 1 < stage_count || stage.out_file.is_some();
        let (status, out_len) = exec_from_bin_io(stage.cmd, args, stdin, capture, &mut capture_buf);
        if status == u64::MAX {
            write_str("unknown command: ");
            write_str(stage.cmd);
            write_str("\n");
            return true;
        }

        if capture {
            if idx + 1 < stage_count {
                pipe_len = out_len;
                pipe_data[..pipe_len].copy_from_slice(&capture_buf[..pipe_len]);
            } else if let Some(path) = stage.out_file {
                let mut path_storage = [core::mem::MaybeUninit::<u8>::uninit(); 128];
                let out_path = match resolve_path(path, &mut path_storage) {
                    Some(p) => p,
                    None => {
                        write_str("redirection error: path too long\n");
                        return true;
                    }
                };
                if libos::write_file(out_path, &capture_buf[..out_len]).is_none() {
                    write_str("redirection error: cannot write ");
                    write_str(out_path);
                    write_str("\n");
                    return true;
                }
            }
        }

        idx += 1;
    }

    true
}

fn parse_stage<'a>(
    stage_tokens: &[Token],
    parsed: &'a [u8],
    allow_in: bool,
    allow_out: bool,
    args_buf: &mut [u8; MAX_ARGS],
) -> Option<StageParse<'a>> {
    let mut words = [""; MAX_STAGE_TOKENS];
    let mut word_count = 0usize;
    let mut in_file = None;
    let mut out_file = None;

    let mut i = 0usize;
    while i < stage_tokens.len() {
        match stage_tokens[i].kind {
            TokenKind::RedirectIn => {
                if !allow_in || in_file.is_some() {
                    write_str("parse error: invalid '<' placement\n");
                    return None;
                }
                i += 1;
                if i >= stage_tokens.len() || stage_tokens[i].kind != TokenKind::Word {
                    write_str("parse error: missing file after '<'\n");
                    return None;
                }
                in_file = Some(token_word(stage_tokens[i], parsed));
            }
            TokenKind::RedirectOut => {
                if !allow_out || out_file.is_some() {
                    write_str("parse error: invalid '>' placement\n");
                    return None;
                }
                i += 1;
                if i >= stage_tokens.len() || stage_tokens[i].kind != TokenKind::Word {
                    write_str("parse error: missing file after '>'\n");
                    return None;
                }
                out_file = Some(token_word(stage_tokens[i], parsed));
            }
            TokenKind::Pipe => {
                write_str("parse error: unexpected '|'\n");
                return None;
            }
            TokenKind::Word => {
                if word_count >= MAX_STAGE_TOKENS {
                    write_str("parse error: too many tokens\n");
                    return None;
                }
                words[word_count] = token_word(stage_tokens[i], parsed);
                word_count += 1;
            }
        }
        i += 1;
    }

    if word_count == 0 {
        write_str("parse error: empty command stage\n");
        return None;
    }

    let cmd = words[0];
    let mut args_len = 0usize;
    let mut j = 1usize;
    while j < word_count {
        let part = words[j].as_bytes();
        if args_len > 0 {
            if args_len >= args_buf.len() {
                write_str("parse error: args too long\n");
                return None;
            }
            args_buf[args_len] = b' ';
            args_len += 1;
        }
        if args_len + part.len() > args_buf.len() {
                write_str("parse error: args too long\n");
                return None;
            }
        args_buf[args_len..args_len + part.len()].copy_from_slice(part);
        args_len += part.len();
        j += 1;
    }

    Some(StageParse {
        cmd,
        args_len,
        in_file,
        out_file,
    })
}

fn tokenize_command(
    line: &str,
    parsed_out: &mut [u8; MAX_PARSED_LINE],
    tokens_out: &mut [Token; MAX_TOKENS],
) -> Result<usize, &'static str> {
    let bytes = line.as_bytes();
    let mut parsed_len = 0usize;
    let mut token_count = 0usize;
    let mut current_start = 0usize;
    let mut in_token = false;
    let mut in_single = false;
    let mut in_double = false;
    let mut escape = false;

    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];

        if escape {
            push_char(parsed_out, &mut parsed_len, b)?;
            in_token = true;
            escape = false;
            i += 1;
            continue;
        }

        if in_single {
            if b == b'\'' {
                in_single = false;
            } else {
                push_char(parsed_out, &mut parsed_len, b)?;
                in_token = true;
            }
            i += 1;
            continue;
        }

        if in_double {
            if b == b'"' {
                in_double = false;
            } else if b == b'\\' {
                escape = true;
            } else {
                push_char(parsed_out, &mut parsed_len, b)?;
                in_token = true;
            }
            i += 1;
            continue;
        }

        match b {
            b' ' | b'\t' => {
                if in_token {
                    push_token(
                        tokens_out,
                        &mut token_count,
                        TokenKind::Word,
                        current_start,
                        parsed_len - current_start,
                    )?;
                    in_token = false;
                }
            }
            b'\'' => {
                if !in_token {
                    current_start = parsed_len;
                    in_token = true;
                }
                in_single = true;
            }
            b'"' => {
                if !in_token {
                    current_start = parsed_len;
                    in_token = true;
                }
                in_double = true;
            }
            b'\\' => {
                if !in_token {
                    current_start = parsed_len;
                    in_token = true;
                }
                escape = true;
            }
            b'|' | b'<' | b'>' => {
                if in_token {
                    push_token(
                        tokens_out,
                        &mut token_count,
                        TokenKind::Word,
                        current_start,
                        parsed_len - current_start,
                    )?;
                    in_token = false;
                }
                let kind = match b {
                    b'|' => TokenKind::Pipe,
                    b'<' => TokenKind::RedirectIn,
                    _ => TokenKind::RedirectOut,
                };
                push_token(tokens_out, &mut token_count, kind, 0, 0)?;
            }
            _ => {
                if !in_token {
                    current_start = parsed_len;
                    in_token = true;
                }
                push_char(parsed_out, &mut parsed_len, b)?;
            }
        }

        i += 1;
    }

    if escape {
        return Err("trailing escape");
    }
    if in_single || in_double {
        return Err("unclosed quote");
    }
    if in_token {
        push_token(
            tokens_out,
            &mut token_count,
            TokenKind::Word,
            current_start,
            parsed_len - current_start,
        )?;
    }

    Ok(token_count)
}

fn push_char(dst: &mut [u8], len: &mut usize, b: u8) -> Result<(), &'static str> {
    if *len >= dst.len() {
        return Err("line too long");
    }
    dst[*len] = b;
    *len += 1;
    Ok(())
}

fn push_token(
    tokens_out: &mut [Token],
    token_count: &mut usize,
    kind: TokenKind,
    start: usize,
    len: usize,
) -> Result<(), &'static str> {
    if *token_count >= tokens_out.len() {
        return Err("too many tokens");
    }
    tokens_out[*token_count] = Token { kind, start, len };
    *token_count += 1;
    Ok(())
}

fn token_word<'a>(token: Token, parsed: &'a [u8]) -> &'a str {
    core::str::from_utf8(&parsed[token.start..token.start + token.len]).unwrap_or("")
}

fn join_word_tokens(tokens: &[Token], parsed: &[u8], out: &mut [u8; MAX_ARGS]) -> Option<usize> {
    let mut out_len = 0usize;
    let mut i = 0usize;
    while i < tokens.len() {
        if tokens[i].kind != TokenKind::Word {
            return None;
        }
        let part = token_word(tokens[i], parsed).as_bytes();
        if out_len > 0 {
            if out_len >= out.len() {
                return None;
            }
            out[out_len] = b' ';
            out_len += 1;
        }
        if out_len + part.len() > out.len() {
            return None;
        }
        out[out_len..out_len + part.len()].copy_from_slice(part);
        out_len += part.len();
        i += 1;
    }
    Some(out_len)
}

fn set_cwd(args: &str) {
    let mut path = args.trim();
    if path.is_empty() {
        path = "/";
    }

    let mut new_cwd = [0u8; 128];
    let mut new_cwd_len;

    if path.starts_with('/') {
        let bytes = path.as_bytes();
        let len = core::cmp::min(bytes.len(), new_cwd.len() - 1);
        new_cwd[..len].copy_from_slice(&bytes[..len]);
        new_cwd_len = len;
    } else {
        if let Some(cwd) = get_env("CWD") {
            let len1 = core::cmp::min(cwd.len(), new_cwd.len() - 1);
            new_cwd[..len1].copy_from_slice(&cwd[..len1]);
            new_cwd_len = len1;
            if new_cwd_len > 0 && new_cwd[new_cwd_len - 1] != b'/' {
                new_cwd[new_cwd_len] = b'/';
                new_cwd_len += 1;
            }
        } else {
            new_cwd[0] = b'/';
            new_cwd_len = 1;
        }
        let bytes = path.as_bytes();
        let len2 = core::cmp::min(bytes.len(), new_cwd.len() - new_cwd_len - 1);
        new_cwd[new_cwd_len..new_cwd_len + len2].copy_from_slice(&bytes[..len2]);
        new_cwd_len += len2;
    }

    if new_cwd_len > 1 && new_cwd[new_cwd_len - 1] == b'/' {
        new_cwd_len -= 1;
    }

    if let Ok(p) = core::str::from_utf8(&new_cwd[..new_cwd_len]) {
        set_env("CWD", p);
    }
}

fn is_builtin(cmd: &str) -> bool {
    matches!(cmd, "help" | "clear" | "exit" | "cd" | "env" | "export")
}

fn resolve_path<'a>(
    path: &str,
    out: &'a mut [core::mem::MaybeUninit<u8>; 128],
) -> Option<&'a str> {
    if path.starts_with('/') {
        let path_bytes = path.as_bytes();
        if path_bytes.len() > out.len() {
            return None;
        }
        let mut i = 0usize;
        while i < path_bytes.len() {
            out[i].write(path_bytes[i]);
            i += 1;
        }
        let bytes = unsafe { core::slice::from_raw_parts(out.as_ptr() as *const u8, path_bytes.len()) };
        return core::str::from_utf8(bytes).ok();
    }

    let cwd = match get_env("CWD") {
        Some(v) => core::str::from_utf8(v).unwrap_or("/"),
        None => "/",
    };

    let mut len = 0usize;

    if cwd == "/" {
        out[len].write(b'/');
        len += 1;
    } else {
        let cwd_bytes = cwd.as_bytes();
        if cwd_bytes.len() + 1 > out.len() {
            return None;
        }
        let mut i = 0usize;
        while i < cwd_bytes.len() {
            out[len].write(cwd_bytes[i]);
            len += 1;
            i += 1;
        }
        if cwd_bytes[cwd_bytes.len() - 1] != b'/' {
            out[len].write(b'/');
            len += 1;
        }
    }

    let path_bytes = path.as_bytes();
    if len + path_bytes.len() > out.len() {
        return None;
    }
    let mut i = 0usize;
    while i < path_bytes.len() {
        out[len + i].write(path_bytes[i]);
        i += 1;
    }
    len += path_bytes.len();

    let bytes = unsafe { core::slice::from_raw_parts(out.as_ptr() as *const u8, len) };
    core::str::from_utf8(bytes).ok()
}

fn exec_from_bin(cmd: &str, args: &str) -> u64 {
    let (status, _) = exec_from_bin_io(cmd, args, None, false, &mut []);
    status
}

fn exec_from_bin_io(
    cmd: &str,
    args: &str,
    stdin: Option<&[u8]>,
    capture: bool,
    capture_buf: &mut [u8],
) -> (u64, usize) {
    let (env_buf, env_len) = build_env_str();
    let env_str = core::str::from_utf8(&env_buf[..env_len]).unwrap_or("");

    let mut full_path_storage = [core::mem::MaybeUninit::<u8>::uninit(); 64];
    let path = match resolve_bin_path(cmd, &mut full_path_storage) {
        Some(p) => p,
        None => return (u64::MAX, 0),
    };

    if stdin.is_none() && !capture {
        return (libos::exec_str_env(path, args, env_str), 0);
    }

    let stdout = if capture { Some(capture_buf) } else { None };
    libos::exec_io_str_env(path, args, env_str, stdin, stdout)
}

fn resolve_bin_path<'a>(
    cmd: &'a str,
    storage: &'a mut [core::mem::MaybeUninit<u8>; 64],
) -> Option<&'a str> {
    if cmd.starts_with('/') {
        return Some(cmd);
    }

    let prefix = BIN_PREFIX.as_bytes();
    let cmd_bytes = cmd.as_bytes();
    let total = prefix.len() + cmd_bytes.len();
    if total == 0 || total > 63 {
        return None;
    }

    let mut i = 0;
    while i < prefix.len() {
        storage[i].write(prefix[i]);
        i += 1;
    }

    let mut j = 0;
    while j < cmd_bytes.len() {
        storage[i + j].write(cmd_bytes[j]);
        j += 1;
    }

    let bytes = unsafe { core::slice::from_raw_parts(storage.as_ptr() as *const u8, total) };
    core::str::from_utf8(bytes).ok()
}
