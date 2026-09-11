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

cargo ndk -t armeabi-v7a -t arm64-v8a -t x86 -t x86_64 --platform 23 --link-builtins build --release --locked

status="$(git status --porcelain --untracked-files=all)"
if [[ -n "$status" ]]; then
    echo "build modified the working tree" >&2
    echo "$status" >&2
    exit 1
fi

mkdir -p dist
armeabi_v7a_artifact="dist/arisa-armeabi-v7a"
arm64_artifact="dist/arisa-arm64-v8a"
x86_artifact="dist/arisa-x86"
x86_64_artifact="dist/arisa-x86_64"
cp target/armv7-linux-androideabi/release/arisa "$armeabi_v7a_artifact"
cp target/aarch64-linux-android/release/arisa "$arm64_artifact"
cp target/i686-linux-android/release/arisa "$x86_artifact"
cp target/x86_64-linux-android/release/arisa "$x86_64_artifact"

gh release create "$tag" \
    "$armeabi_v7a_artifact" \
    "$arm64_artifact" \
    "$x86_artifact" \
    "$x86_64_artifact" \
    "scripts/arisa_control" \
    "scripts/arisa_control.ps1" \
    "prebuilts/fileprovider.apk" \
    --repo huni-ee/arisa2 \
    --target "$commit" \
    --title "arisa $tag" \
    --generate-notes
