#!/usr/bin/env bash
# Build and test an installed wheel against the native archive from this job.
set -euo pipefail
artifact_root=${CARGO_TARGET_DIR:-target}
build_target=${1:-}
build_args=()
if command -v cygpath >/dev/null 2>&1 && [ -n "${pythonLocation:-}" ]; then
  # MSYS2 starts with a restricted PATH; setup-python exposes its native path.
  python_bin=$(cygpath -u "$pythonLocation")
  export PATH="$python_bin:$python_bin/Scripts:$PATH"
fi
if [ -n "$build_target" ]; then
  artifact_root="$artifact_root/$build_target"
  build_args+=(--target "$build_target")
fi
libraries=()
for library in "$artifact_root/debug"/build/feff10-sys-*/out/libfeff10.a; do
  [ -f "$library" ] && libraries+=("$library")
done
if [ "${#libraries[@]}" -ne 1 ]; then
  echo "Expected one source-built FEFF archive; found ${#libraries[@]}" >&2
  exit 1
fi
libdir=$(cd "$(dirname "${libraries[0]}")" && pwd)
if command -v cygpath >/dev/null 2>&1; then
  libdir=$(cygpath -m "$libdir")
fi
export FEFF10_LIB_DIR="$libdir"
wheel_dir=$(mktemp -d)
trap 'rm -rf "$wheel_dir"' EXIT
python -m pip install maturin pytest
python -m maturin build --manifest-path crates/feff10-python/Cargo.toml \
  --features prebuilt -i python --out "$wheel_dir" "${build_args[@]}"
if command -v cygpath >/dev/null 2>&1; then
  python -m pip install delvewheel
  python -m delvewheel repair "$wheel_dir"/*.whl --wheel-dir "$wheel_dir/repaired" \
    --add-path "$(cygpath -m /mingw64/bin)" --no-mangle-all
  python -m pip install --force-reinstall "$wheel_dir"/repaired/*.whl
else
  python -m pip install --force-reinstall "$wheel_dir"/*.whl
fi
python -m pytest crates/feff10-python/tests -q
