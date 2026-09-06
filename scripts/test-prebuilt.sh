#!/usr/bin/env bash
# Test the just-built archive through the same prebuilt feature consumers use.
set -euo pipefail
profile=${1:-release}
build_target=${2:-}
artifact_root=${CARGO_TARGET_DIR:-target}
cargo_args=(-p feff10 --features prebuilt)
if [ -n "$build_target" ]; then
  artifact_root="$artifact_root/$build_target"
  cargo_args+=(--target "$build_target")
fi
if [ "$profile" = release ]; then
  cargo_args+=(--release)
fi
libraries=()
for library in "$artifact_root/$profile"/build/feff10-sys-*/out/libfeff10.a; do
  [ -f "$library" ] && libraries+=("$library")
done
if [ "${#libraries[@]}" -ne 1 ]; then
  echo "Expected one freshly built FEFF archive; found ${#libraries[@]} in $artifact_root/$profile" >&2
  exit 1
fi
libdir=$(cd "$(dirname "${libraries[0]}")" && pwd)
if command -v cygpath >/dev/null 2>&1; then
  libdir=$(cygpath -m "$libdir")
fi
FEFF10_LIB_DIR="$libdir" cargo test "${cargo_args[@]}" --test worker_mode
