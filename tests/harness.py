#!/usr/bin/env python3
import argparse
import json
import os
import select
import socket
import subprocess
import sys
import time
from pathlib import Path

try:
    from PIL import Image
    import numpy as np
except Exception:
    Image = None
    np = None

ROOT = Path(__file__).resolve().parent.parent
OUT_DIR = ROOT / "tests" / "output"
OUT_DIR.mkdir(parents=True, exist_ok=True)

QEMU_CMD = [
    "qemu-system-x86_64",
    "-drive", "format=raw,file=target/x86_64-smultron/debug/bootimage-kernel.bin",
    "-serial", "stdio",
    "-display", "none",
    "-qmp", "tcp:127.0.0.1:4444,server,nowait",
]

PHASE_MARKERS = [
    "[ok][phase2]",
    "[ok][phase3]",
    "[ok][phase4]",
    "[ok][phase5]",
    "[ok][phase6]",
    "[ok][phase7]",
    "[ok][phase8]",
]

SHELL_MARKERS = [
    "[ok] launching userspace init shell",
    "smultron shell (userspace)",
    "smultron$",
]


def wait_for_serial_markers(timeout=20):
    proc = subprocess.Popen(
        QEMU_CMD,
        cwd=ROOT,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        bufsize=1,
    )
    observed = []
    start = time.time()
    try:
        while time.time() - start < timeout:
            ready, _, _ = select.select([proc.stdout], [], [], 0.2)
            if not ready:
                continue
            line = proc.stdout.readline()
            if not line:
                continue
            observed.append(line.rstrip())
            for marker in PHASE_MARKERS:
                if marker in line and marker not in observed:
                    observed.append(marker)
            if all(any(m in o for o in observed) for m in PHASE_MARKERS):
                return True, observed, proc
        return False, observed, proc
    except Exception as exc:
        return False, [f"exception: {exc}"], proc


def wait_for_markers(markers, timeout=20):
    proc = subprocess.Popen(
        QEMU_CMD,
        cwd=ROOT,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=False,
        bufsize=0,
    )
    observed = []
    serial_text = ""
    start = time.time()
    try:
        while time.time() - start < timeout:
            ready, _, _ = select.select([proc.stdout], [], [], 0.2)
            if not ready:
                continue
            chunk = os.read(proc.stdout.fileno(), 4096)
            if not chunk:
                continue
            decoded = chunk.decode(errors="replace")
            serial_text += decoded
            while "\n" in serial_text:
                line, serial_text = serial_text.split("\n", 1)
                observed.append(line.rstrip("\r"))
            if all(m in serial_text or any(m in o for o in observed) for m in markers):
                if serial_text.strip():
                    observed.append(serial_text.rstrip("\r"))
                return True, observed, proc
        if serial_text.strip():
            observed.append(serial_text.rstrip("\r"))
        return False, observed, proc
    except Exception as exc:
        return False, [f"exception: {exc}"], proc


def qmp_cmd(cmd):
    s = socket.create_connection(("127.0.0.1", 4444), timeout=2)
    try:
        _ = s.recv(4096)
        s.sendall(b'{"execute":"qmp_capabilities"}\r\n')
        _ = s.recv(4096)
        s.sendall((json.dumps(cmd) + "\r\n").encode())
        resp = s.recv(4096)
        return resp.decode(errors="replace")
    finally:
        s.close()


def stop_qemu(proc):
    if proc is None or proc.poll() is not None:
        return
    proc.terminate()
    try:
        proc.wait(timeout=2)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait(timeout=2)


def visual_check(path):
    if Image is None or np is None:
        return False, "Pillow/numpy unavailable"
    img = Image.open(path).convert("RGB")
    arr = np.array(img)
    non_black = np.count_nonzero(arr)
    if non_black > 100:
        return True, f"non-black pixels: {non_black}"
    return False, f"insufficient drawn pixels: {non_black}"


def mode_no_qemu():
    try:
        qmp_cmd({"execute": "query-status"})
        print("[failed] unexpected QMP response when QEMU should be absent")
        return 1
    except Exception as exc:
        print(f"[ok] expected failure when QEMU absent: {exc}")
        return 0


def mode_phase_all():
    ok, lines, proc = wait_for_serial_markers()
    for line in lines[-80:]:
        print(line)
    if not ok:
        print("[failed] serial markers not satisfied")
        stop_qemu(proc)
        return 1

    ppm = OUT_DIR / "screen.ppm"
    try:
        resp = qmp_cmd({"execute": "screendump", "arguments": {"filename": str(ppm)}})
        print(f"QMP: {resp.strip()}")
    except Exception as exc:
        print(f"[failed] qmp screendump failed: {exc}")
        stop_qemu(proc)
        return 1

    vok, detail = visual_check(ppm)
    print(f"visual_check: {detail}")
    stop_qemu(proc)
    if not vok:
        print("[failed] visual verification failed")
        return 1

    print("[ok] functional + visual checks passed")
    return 0


def mode_shell_start():
    ok, lines, proc = wait_for_markers(SHELL_MARKERS)
    for line in lines[-120:]:
        print(line)
    stop_qemu(proc)
    if not ok:
        print("[failed] shell markers not satisfied")
        return 1
    print("[ok] shell markers observed")
    return 0


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--mode",
        choices=["no-qemu", "phase-all", "shell-start"],
        default="phase-all",
    )
    args = parser.parse_args()

    if args.mode == "no-qemu":
        sys.exit(mode_no_qemu())
    if args.mode == "shell-start":
        sys.exit(mode_shell_start())
    sys.exit(mode_phase_all())


if __name__ == "__main__":
    main()
