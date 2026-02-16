#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
SNAPSHOT_SCRIPT="${REPO_ROOT}/scripts/fortran/generate-baseline-snapshots.sh"

assert_file() {
  local path="$1"
  if [[ ! -f "${path}" ]]; then
    echo "ASSERTION FAILED: expected file ${path}" >&2
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

assert_grep() {
  local pattern="$1"
  local path="$2"
  if ! grep -q -- "${pattern}" "${path}"; then
    echo "ASSERTION FAILED: expected pattern '${pattern}' in ${path}" >&2
    exit 1
  fi
}

run_test_generates_snapshots_with_checksums() {
  local temp_root="$1"
  local fixture_archive="${temp_root}/fixtures/fx-archive"
  local fixture_file="${temp_root}/fixtures/fx-file"
  local reference_file="${fixture_file}/referenceherfd.dat"
  local runner_path="${temp_root}/runner.sh"
  local archive_content="${temp_root}/archive-content"
  local archive_path="${temp_root}/reference.zip"
  local manifest_path="${temp_root}/manifest.json"
  local output_root="${temp_root}/snapshots"

  mkdir -p "${fixture_archive}" "${fixture_file}" "${archive_content}/REFERENCE"
  printf 'TITLE\n' > "${fixture_archive}/feff.inp"
  printf 'TITLE\n' > "${fixture_file}/feff.inp"
  printf '0.1 1.2\n' > "${archive_content}/REFERENCE/xmu.dat"
  printf 'reference spectrum\n' > "${reference_file}"

  (
    cd "${archive_content}" || exit 1
    zip -q -r "${archive_path}" REFERENCE
  )

  cat > "${runner_path}" <<'RUNNER'
#!/usr/bin/env bash
set -euo pipefail
printf 'runner output\n' > runner.out
RUNNER
  chmod +x "${runner_path}"

  cat > "${manifest_path}" <<EOF_MANIFEST
{
  "fixtures": [
    {
      "id": "FX-ARCHIVE-001",
      "inputDirectory": "${fixture_archive}",
      "entryFiles": ["feff.inp"],
      "baselineSources": [
        {"kind": "reference_archive", "path": "${archive_path}"}
      ],
      "baselineStatus": "reference_archive_available"
    },
    {
      "id": "FX-FILE-001",
      "inputDirectory": "${fixture_file}",
      "entryFiles": ["feff.inp"],
      "baselineSources": [
        {"kind": "reference_file", "path": "${reference_file}"}
      ],
      "baselineStatus": "reference_files_available"
    }
  ]
}
EOF_MANIFEST

  "${SNAPSHOT_SCRIPT}" \
    --manifest "${manifest_path}" \
    --output-root "${output_root}" \
    --capture-runner "${runner_path}"

  assert_dir "${output_root}/FX-ARCHIVE-001/baseline"
  assert_file "${output_root}/FX-ARCHIVE-001/baseline/xmu.dat"
  assert_file "${output_root}/FX-ARCHIVE-001/baseline/runner.out"
  assert_file "${output_root}/FX-ARCHIVE-001/checksums.sha256"
  assert_file "${output_root}/FX-ARCHIVE-001/snapshot-metadata.json"
  assert_grep 'xmu.dat' "${output_root}/FX-ARCHIVE-001/checksums.sha256"

  assert_dir "${output_root}/FX-FILE-001/baseline"
  assert_file "${output_root}/FX-FILE-001/baseline/referenceherfd.dat"
  assert_file "${output_root}/FX-FILE-001/checksums.sha256"
  assert_file "${output_root}/FX-FILE-001/snapshot-metadata.json"
  assert_grep 'referenceherfd.dat' "${output_root}/FX-FILE-001/checksums.sha256"

  assert_file "${output_root}/snapshot-index.json"
  local fixture_count
  fixture_count="$(jq -r '.fixtureCount' "${output_root}/snapshot-index.json")"
  if [[ "${fixture_count}" != "2" ]]; then
    echo "ASSERTION FAILED: expected fixtureCount=2, got ${fixture_count}" >&2
    exit 1
  fi
}

main() {
  if [[ ! -x "${SNAPSHOT_SCRIPT}" ]]; then
    echo "Missing snapshot script: ${SNAPSHOT_SCRIPT}" >&2
    exit 1
  fi

  if ! command -v zip >/dev/null 2>&1; then
    echo "Missing required tool: zip" >&2
    exit 1
  fi

  local temp_root
  temp_root="$(mktemp -d)"
  trap "rm -rf '${temp_root}'" EXIT

  run_test_generates_snapshots_with_checksums "${temp_root}"

  echo 'test-generate-baseline-snapshots: PASS'
}

main "$@"
