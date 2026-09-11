#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_dir"

cargo ndk --version >/dev/null
cargo ndk -t armeabi-v7a -t arm64-v8a -t x86 -t x86_64 --platform 23 --link-builtins build --release --locked


mkdir -p dist
cp target/armv7-linux-android/release/arisa "dist/arisa-armeabi-v7a"
cp target/aarch64-linux-android/release/arisa dist/arisa-arm64-v8a
cp target/i686-linux-android/release/arisa dist/arisa-x86
cp target/x86_64-linux-android/release/arisa dist/arisa-x86_64