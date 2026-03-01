KERNEL_DIR := kernel
PYTHON := python3

.PHONY: build run test harness-fail initrd

build:
	cd $(KERNEL_DIR) && cargo bootimage

run:
	cd $(KERNEL_DIR) && cargo run

harness-fail:
	$(PYTHON) tests/harness.py --mode no-qemu

test:
	$(PYTHON) tests/harness.py --mode phase-all

initrd:
	bash tools/mk_initrd.sh
