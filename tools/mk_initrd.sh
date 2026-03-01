#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BUILD_DIR="$ROOT/build/initrd"
OUT="$ROOT/build/initrd.tar"

rm -rf "$BUILD_DIR"
mkdir -p "$BUILD_DIR"

echo "hello.txt from initrd/TarFS" > "$BUILD_DIR/hello.txt"
# Placeholder for a future real userspace ELF binary.
cp "$ROOT/userspace/apps/init/src/main.rs" "$BUILD_DIR/init"

tar -cf "$OUT" -C "$BUILD_DIR" .
echo "Generated $OUT"
