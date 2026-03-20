# Smultron OS

Smultron is an entirely vibe-coded x86_64 Rust OS kernel with a small userspace loaded as separate ELF binaries at runtime. This line of text is the only user-written text in the entire repository.

## Current Scope

- Boots in QEMU with serial and VGA output.
- Kernel/user split is in place, but userspace execution is still ring0-backed (no full ring3 isolation yet).
- Userspace shell app (`/bin/init`) supports builtins, launching `/bin/*` programs, env vars, quoting, pipes, and basic redirection.

## Implemented Userspace Apps

- `/bin/init` (shell)
- `/bin/echo`
- `/bin/env`
- `/bin/ls`
- `/bin/cat`
- `/bin/tee`
- `/bin/sed`

## Prerequisites

- Linux environment with `qemu-system-x86_64`
- Rust nightly + components:
  - `rust-src`
  - `llvm-tools-preview`
- `cargo-bootimage` installed
- Python 3 + test deps from `tests/requirements.txt` (includes `pytest`, `Pillow`, `numpy`)

## Build And Run

From repository root:

```bash
# Build boot image
cd kernel && cargo bootimage

# Run in QEMU
cd kernel && cargo run
```

Makefile shortcuts:

```bash
make build   # kernel boot image
make run     # boot in qemu
make test    # harness phase-all
```

## Verification Gates

Run gates sequentially (not in parallel) because QMP uses fixed port `127.0.0.1:4444`.

```bash
python3 tests/harness.py --mode phase-all
pytest -q
cd kernel && cargo bootimage
```

Additional shell-start gate:

```bash
python3 tests/harness.py --mode shell-start
```

## Shell Usage

Builtins:

- `help`
- `clear`
- `exit`
- `cd <path>`
- `env`
- `export KEY=VALUE`

External commands:

- `echo ...`
- `env`
- `ls`
- `cat <file>`
- `tee <file> [file2 ...]`
- `sed s/old/new/[g] [file...]`

I/O features:

- Pipe: `cat /hello.txt | tee /tmp.txt`
- Redirect out: `echo hello > /note.txt`
- Redirect in: `cat < /hello.txt`
- Quotes: `echo "hello world"` and `echo 'a b c'`

## Repository Layout

- `kernel/` kernel crate and bootimage target config
- `userspace/libos/` syscall wrapper library for userspace apps
- `userspace/apps/*` userspace command binaries
- `tests/harness.py` QEMU functional/visual gate harness
- `tests/test_harness.py` pytest coverage for harness behavior
- `OS_STATE.md` ongoing implementation state, verification logs, and known regressions
- `AGENTS.md` mandatory repository working rules

## Known Gaps

- Live `int3` path is still unstable in mapped-heap configuration (`#GP -> #DF` chain).
- External IRQ/timer handling remains deferred.
- `syscall/sysret` trampoline context switching is scaffolded, not complete.
- No scheduler/preemption/fork/wait yet.
- No per-process page-table isolation yet.

See `OS_STATE.md` for exact failure signatures and latest verification records.
