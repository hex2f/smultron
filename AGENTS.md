# AGENTS Rules (Smultron OS)

These are mandatory working rules for the coding agent in this repository.

## 1) State Discipline
- Read `OS_STATE.md` before any major implementation step.
- Update `OS_STATE.md` after each major step with:
  - what changed,
  - what was verified,
  - known regressions and exact failure signatures.
- Do not claim a milestone is complete unless its automated gate passes.

## 2) Verification Gates
- Do not advance to the next major milestone without automated verification.
- Required commands for verification:
  - `python3 tests/harness.py --mode phase-all`
  - `pytest -q`
- For build verification of boot image:
  - `cd kernel && cargo bootimage`

## 3) Failure Honesty
- Never report success when tests fail.
- On QEMU/runtime failure, capture and report concrete evidence:
  - serial output markers,
  - QEMU debug traces/logs,
  - register/fault details when available.

## 4) Debugging Workflow
- Prefer minimal, reversible fixes.
- If a fix regresses bootability, restore stable behavior and log the regression in `OS_STATE.md`.
- Keep unresolved low-level issues explicitly tracked with exact fault chains and addresses.

## 5) Architecture Intent
- Preserve strict kernel/user separation intent.
- Keep syscall ABI aligned with System V AMD64 register convention (`rdi`, `rsi`, `rdx`, `r10`, `r8`, `r9`).
- Maintain non-monolithic direction: user programs as separate ELF artifacts loaded at runtime.

## 6) Safety for Existing Work
- Do not remove or overwrite working harness infrastructure unless replacing it with equivalent or better automated coverage.
- Do not silently change milestone semantics; record deviations in `OS_STATE.md`.

## 7) Communication Style
- Be concise and explicit.
- State assumptions when behavior is placeholder/scaffolded.
- Distinguish clearly between:
  - implemented and verified,
  - implemented but unverified,
  - planned only.
