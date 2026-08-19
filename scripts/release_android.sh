#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_dir"

status="$(git status --porcelain --untracked-files=all)"
if [[ -n "$status" ]]; then
    echo "working tree is not clean" >&2
    echo "$status" >&2
    exit 1
fi

if [[ "$(git branch --show-current)" != "main" ]]; then
    echo "release must be created from main" >&2
    exit 1
fi

cargo ndk --version >/dev/null
gh auth status >/dev/null

git fetch --quiet origin main
commit="$(git rev-parse HEAD)"
if [[ "$commit" != "$(git rev-parse origin/main)" ]]; then
    echo "HEAD does not match origin/main" >&2
    exit 1
fi

package_id="$(cargo pkgid)"
version="${package_id##*@}"
tag="v$version"

if gh release view "$tag" --repo huni-ee/arisa2 >/dev/null 2>&1; then
    echo "release $tag already exists" >&2
    exit 1
fi

if git ls-remote --exit-code --tags origin "refs/tags/$tag" >/dev/null 2>&1; then
    echo "tag $tag already exists" >&2
    exit 1
fi

cargo ndk \
    -t armeabi-v7a \
    -t arm64-v8a \
    -t x86 \
    -t x86_64 \
    -P 23 \
    --link-builtins build --release --locked

status="$(git status --porcelain --untracked-files=all)"
if [[ -n "$status" ]]; then
    echo "build modified the working tree" >&2
    echo "$status" >&2
    exit 1
fi

mkdir -p dist
cp target/armv7-linux-androideabi/release/arisa dist/arisa-armeabi-v7a
cp target/aarch64-linux-android/release/arisa dist/arisa-arm64-v8a
cp target/i686-linux-android/release/arisa dist/arisa-x86
cp target/x86_64-linux-android/release/arisa dist/arisa-x86_64

gh release create "$tag" \
    dist/arisa-armeabi-v7a \
    dist/arisa-arm64-v8a \
    dist/arisa-x86 \
    dist/arisa-x86_64 \
    scripts/arisa_control \
    scripts/arisa_control.ps1 \
    fileprovider.apk \
    --repo huni-ee/arisa2 \
    --target "$commit" \
    --title "arisa $tag" \
    --generate-notes
