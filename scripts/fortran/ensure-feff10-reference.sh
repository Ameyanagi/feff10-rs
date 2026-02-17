#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

log() {
  printf '[ensure-feff10-reference] %s\n' "$*"
}

err() {
  printf '[ensure-feff10-reference] ERROR: %s\n' "$*" >&2
}

reference_repo="${FEFF10_REFERENCE_REPO:-https://github.com/times-software/feff10}"
reference_ref="${FEFF10_REFERENCE_REF:-master}"
reference_dir="${FEFF10_REFERENCE_DIR:-${REPO_ROOT}/feff10}"

if [[ "${reference_dir}" != /* ]]; then
  reference_dir="${REPO_ROOT}/${reference_dir}"
fi

if [[ -e "${reference_dir}" && ! -d "${reference_dir}/.git" ]]; then
  err "Reference path exists but is not a git checkout: ${reference_dir}"
  exit 1
fi

if [[ -d "${reference_dir}/.git" ]]; then
  log "Using existing FEFF10 checkout at ${reference_dir}"
else
  log "Cloning FEFF10 reference checkout from ${reference_repo} (branch: ${reference_ref})"
  git clone --depth 1 --branch "${reference_ref}" "${reference_repo}" "${reference_dir}"
fi

license_path="${reference_dir}/LICENSE"
if [[ ! -f "${license_path}" ]]; then
  err "FEFF10 reference checkout is missing LICENSE: ${license_path}"
  exit 1
fi

log "Reference checkout ready: ${reference_dir}"
log "License file found: ${license_path}"
