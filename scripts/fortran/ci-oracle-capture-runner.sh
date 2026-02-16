#!/usr/bin/env bash
set -euo pipefail

log() {
  printf '[ci-oracle-capture-runner] %s\n' "$*"
}

err() {
  printf '[ci-oracle-capture-runner] ERROR: %s\n' "$*" >&2
}

workspace_root="${GITHUB_WORKSPACE:-}"
if [[ -z "${workspace_root}" ]]; then
  workspace_root="$(git rev-parse --show-toplevel 2>/dev/null || true)"
fi
if [[ -z "${workspace_root}" ]]; then
  err "Unable to determine repository root (set GITHUB_WORKSPACE or run inside a git checkout)."
  exit 1
fi

capture_outputs_dir="$(pwd)"
fixture_id="$(basename "$(dirname "${capture_outputs_dir}")")"
baseline_dir="${workspace_root}/artifacts/fortran-baselines/${fixture_id}/baseline"

if [[ ! -d "${baseline_dir}" ]]; then
  err "Baseline snapshot directory for fixture '${fixture_id}' was not found: ${baseline_dir}"
  exit 1
fi

cp -R "${baseline_dir}/." "${capture_outputs_dir}/"
log "Seeded oracle capture outputs for fixture '${fixture_id}' from committed baselines."
