import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def test_no_qemu_failure_mode_reports_ok():
    proc = subprocess.run(
        ["python3", "tests/harness.py", "--mode", "no-qemu"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        timeout=20,
    )
    assert proc.returncode == 0
    assert "[ok] expected failure when QEMU absent" in proc.stdout


def test_shell_start_mode_reports_shell_markers():
    proc = subprocess.run(
        ["python3", "tests/harness.py", "--mode", "shell-start"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert proc.returncode == 0
    assert "[ok] shell markers observed" in proc.stdout
