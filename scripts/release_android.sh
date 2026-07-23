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

if gh release view "$tag" --repo ye-seola/arisa2 >/dev/null 2>&1; then
    echo "release $tag already exists" >&2
    exit 1
fi

if git ls-remote --exit-code --tags origin "refs/tags/$tag" >/dev/null 2>&1; then
    echo "tag $tag already exists" >&2
    exit 1
fi

cargo ndk -t arm64-v8a -p 23 build --release --locked

status="$(git status --porcelain --untracked-files=all)"
if [[ -n "$status" ]]; then
    echo "build modified the working tree" >&2
    echo "$status" >&2
    exit 1
fi

mkdir -p dist
artifact="dist/arisa-arm64-v8a"
cp target/aarch64-linux-android/release/arisa "$artifact"

gh release create "$tag" "$artifact" \
    --repo ye-seola/arisa2 \
    --target "$commit" \
    --title "arisa $tag" \
    --generate-notes
