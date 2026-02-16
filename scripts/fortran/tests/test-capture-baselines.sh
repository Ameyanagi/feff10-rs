#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
CAPTURE_SCRIPT="${REPO_ROOT}/scripts/fortran/capture-baselines.sh"

assert_file() {
  local path="$1"
  if [[ ! -f "${path}" ]]; then
    echo "ASSERTION FAILED: expected file ${path}" >&2
    exit 1
  fi
}

assert_not_file() {
  local path="$1"
  if [[ -f "${path}" ]]; then
    echo "ASSERTION FAILED: file should not exist ${path}" >&2
    exit 1
  fi
}

assert_dir() {
  local path="$1"
  if [[ ! -d "${path}" ]]; then
    echo "ASSERTION FAILED: expected directory ${path}" >&2
    exit 1
  fi
}

run_test_default_selection_and_deterministic_tree() {
  local temp_root="$1"
  local fixture_ok="${temp_root}/fixtures/fx-ok"
  local fixture_skip="${temp_root}/fixtures/fx-skip"
  local manifest_path="${temp_root}/manifest-default.json"
  local output_root="${temp_root}/out-default"
  local runner_path="${temp_root}/runner.sh"

  mkdir -p "${fixture_ok}" "${fixture_skip}"
  printf 'TITLE\n' > "${fixture_ok}/feff.inp"
  printf 'TITLE\n' > "${fixture_skip}/feff.inp"

  cat > "${runner_path}" <<'RUNNER'
#!/usr/bin/env bash
set -euo pipefail
printf 'runner invoked\n'
printf 'simulated output\n' > xmu.dat
RUNNER
  chmod +x "${runner_path}"

  cat > "${manifest_path}" <<EOF_MANIFEST
{
  "fixtures": [
    {
      "id": "FX-OK-001",
      "inputDirectory": "${fixture_ok}",
      "entryFiles": ["feff.inp"],
      "baselineSources": [],
      "baselineStatus": "requires_fortran_capture"
    },
    {
      "id": "FX-SKIP-001",
      "inputDirectory": "${fixture_skip}",
      "entryFiles": ["feff.inp"],
      "baselineSources": [],
      "baselineStatus": "reference_archive_available"
    }
  ]
}
EOF_MANIFEST

  "${CAPTURE_SCRIPT}" \
    --manifest "${manifest_path}" \
    --output-root "${output_root}" \
    --runner "${runner_path}"

  assert_dir "${output_root}/FX-OK-001"
  assert_file "${output_root}/FX-OK-001/inputs/feff.inp"
  assert_file "${output_root}/FX-OK-001/outputs/feff.inp"
  assert_file "${output_root}/FX-OK-001/outputs/xmu.dat"
  assert_file "${output_root}/FX-OK-001/logs/runner.log"
  assert_not_file "${output_root}/FX-SKIP-001/metadata.txt"

  printf 'stale\n' > "${output_root}/FX-OK-001/outputs/stale.tmp"

  "${CAPTURE_SCRIPT}" \
    --manifest "${manifest_path}" \
    --output-root "${output_root}" \
    --runner "${runner_path}"

  assert_not_file "${output_root}/FX-OK-001/outputs/stale.tmp"
}

run_test_nonzero_on_failure() {
  local temp_root="$1"
  local fixture_fail="${temp_root}/fixtures/fx-fail"
  local manifest_path="${temp_root}/manifest-fail.json"
  local output_root="${temp_root}/out-fail"
  local runner_path="${temp_root}/runner-fail.sh"

  mkdir -p "${fixture_fail}"
  printf 'TITLE\n' > "${fixture_fail}/feff.inp"
  printf '1\n' > "${fixture_fail}/FAIL_MARKER"

  cat > "${runner_path}" <<'RUNNER'
#!/usr/bin/env bash
set -euo pipefail
if [[ -f FAIL_MARKER ]]; then
  echo 'simulated fixture failure' >&2
  exit 22
fi
printf 'ok\n' > xmu.dat
RUNNER
  chmod +x "${runner_path}"

  cat > "${manifest_path}" <<EOF_MANIFEST
{
  "fixtures": [
    {
      "id": "FX-FAIL-001",
      "inputDirectory": "${fixture_fail}",
      "entryFiles": ["feff.inp", "FAIL_MARKER"],
      "baselineSources": [],
      "baselineStatus": "requires_fortran_capture"
    }
  ]
}
EOF_MANIFEST

  set +e
  "${CAPTURE_SCRIPT}" \
    --manifest "${manifest_path}" \
    --output-root "${output_root}" \
    --runner "${runner_path}" \
    --fixture FX-FAIL-001
  local status=$?
  set -e

  if [[ "${status}" -eq 0 ]]; then
    echo 'ASSERTION FAILED: capture script should exit non-zero when a fixture fails' >&2
    exit 1
  fi

  assert_file "${output_root}/FX-FAIL-001/logs/runner.log"
  if ! grep -q 'simulated fixture failure' "${output_root}/FX-FAIL-001/logs/runner.log"; then
    echo 'ASSERTION FAILED: failure message was not captured in runner.log' >&2
    exit 1
  fi
}

run_test_allow_missing_entry_files() {
  local temp_root="$1"
  local fixture_missing="${temp_root}/fixtures/fx-missing-entry"
  local manifest_path="${temp_root}/manifest-missing-entry.json"
  local output_root="${temp_root}/out-missing-entry"
  local runner_path="${temp_root}/runner-missing-entry.sh"

  mkdir -p "${fixture_missing}"
  printf 'TITLE\n' > "${fixture_missing}/feff.inp"

  cat > "${runner_path}" <<'RUNNER'
#!/usr/bin/env bash
set -euo pipefail
printf 'runner with missing entry support\n'
printf 'baseline\n' > baseline.dat
RUNNER
  chmod +x "${runner_path}"

  cat > "${manifest_path}" <<EOF_MANIFEST
{
  "fixtures": [
    {
      "id": "FX-MISSING-ENTRY-001",
      "inputDirectory": "${fixture_missing}",
      "entryFiles": ["feff.inp", "missing-entry.inp"],
      "baselineSources": [],
      "baselineStatus": "requires_fortran_capture"
    }
  ]
}
EOF_MANIFEST

  "${CAPTURE_SCRIPT}" \
    --manifest "${manifest_path}" \
    --output-root "${output_root}" \
    --runner "${runner_path}" \
    --fixture FX-MISSING-ENTRY-001 \
    --allow-missing-entry-files

  assert_file "${output_root}/FX-MISSING-ENTRY-001/outputs/baseline.dat"
  if ! grep -q '^missing_entry_files=missing-entry.inp$' "${output_root}/FX-MISSING-ENTRY-001/metadata.txt"; then
    echo 'ASSERTION FAILED: missing entry metadata was not recorded' >&2
    exit 1
  fi
}

main() {
  if [[ ! -x "${CAPTURE_SCRIPT}" ]]; then
    echo "Missing capture script: ${CAPTURE_SCRIPT}" >&2
    exit 1
  fi

  local temp_root
  temp_root="$(mktemp -d)"
  trap "rm -rf '${temp_root}'" EXIT

  run_test_default_selection_and_deterministic_tree "${temp_root}"
  run_test_nonzero_on_failure "${temp_root}"
  run_test_allow_missing_entry_files "${temp_root}"

  echo 'test-capture-baselines: PASS'
}

main "$@"
