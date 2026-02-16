#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
CAPTURE_SCRIPT="${SCRIPT_DIR}/capture-baselines.sh"

log() {
  printf '[generate-baseline-snapshots] %s\n' "$*"
}

err() {
  printf '[generate-baseline-snapshots] ERROR: %s\n' "$*" >&2
}

usage() {
  cat <<'USAGE'
Usage:
  scripts/fortran/generate-baseline-snapshots.sh [options]

Options:
  --manifest <path>        Fixture manifest JSON path (default: tasks/golden-fixture-manifest.json)
  --output-root <path>     Snapshot output root (default: artifacts/fortran-baselines)
  --capture-runner <cmd>   Runner command passed to capture-baselines (default: :)
  --capture-bin-dir <path> Module binary directory passed to capture-baselines
  --help                   Show this help text

Notes:
- This command always captures all fixtures from the manifest.
- It records per-fixture checksums in checksums.sha256 and snapshot metadata in snapshot-metadata.json.
USAGE
}

resolve_path() {
  local raw_path="$1"

  if [[ "${raw_path}" = /* ]]; then
    printf '%s\n' "${raw_path}"
    return
  fi

  if [[ -e "${raw_path}" ]]; then
    printf '%s/%s\n' "$(cd "$(dirname "${raw_path}")" && pwd)" "$(basename "${raw_path}")"
    return
  fi

  printf '%s/%s\n' "${REPO_ROOT}" "${raw_path}"
}

require_tool() {
  local tool_name="$1"
  if ! command -v "${tool_name}" >/dev/null 2>&1; then
    err "Required tool '${tool_name}' was not found in PATH."
    exit 1
  fi
}

sha256_file() {
  local file_path="$1"

  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "${file_path}" | awk '{print $1}'
    return
  fi

  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "${file_path}" | awk '{print $1}'
    return
  fi

  err "No SHA-256 tool found (expected sha256sum or shasum)."
  exit 1
}

extract_archive_into_baseline() {
  local archive_path="$1"
  local baseline_dir="$2"
  local entry
  local rel_path
  local has_reference_prefix=0

  declare -a archive_entries=()
  while IFS= read -r entry; do
    [[ -z "${entry}" ]] && continue
    archive_entries+=("${entry}")
  done < <(unzip -Z1 "${archive_path}")

  has_reference_prefix=0
  for entry in "${archive_entries[@]}"; do
    if [[ "${entry}" == REFERENCE/* ]]; then
      has_reference_prefix=1
      break
    fi
  done

  for entry in "${archive_entries[@]}"; do
    case "${entry}" in
      */)
        continue
        ;;
      __MACOSX/*|*/.DS_Store|.DS_Store|*/._*)
        continue
        ;;
    esac

    if [[ "${has_reference_prefix}" -eq 1 ]]; then
      if [[ "${entry}" != REFERENCE/* ]]; then
        continue
      fi
      rel_path="${entry#REFERENCE/}"
    else
      rel_path="${entry}"
    fi

    [[ -z "${rel_path}" ]] && continue

    mkdir -p "${baseline_dir}/$(dirname "${rel_path}")"
    unzip -p "${archive_path}" "${entry}" > "${baseline_dir}/${rel_path}"
  done
}

copy_capture_outputs() {
  local capture_outputs_dir="$1"
  local baseline_dir="$2"

  if [[ ! -d "${capture_outputs_dir}" ]]; then
    return
  fi

  (
    shopt -s dotglob nullglob
    local entries=("${capture_outputs_dir}"/*)
    if [[ "${#entries[@]}" -eq 0 ]]; then
      return
    fi
    cp -R "${entries[@]}" "${baseline_dir}/"
  )
}

manifest_path='tasks/golden-fixture-manifest.json'
output_root='artifacts/fortran-baselines'
capture_runner=''
capture_bin_dir=''

while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --manifest)
      manifest_path="$2"
      shift 2
      ;;
    --output-root)
      output_root="$2"
      shift 2
      ;;
    --capture-runner)
      capture_runner="$2"
      shift 2
      ;;
    --capture-bin-dir)
      capture_bin_dir="$2"
      shift 2
      ;;
    --help)
      usage
      exit 0
      ;;
    *)
      err "Unknown argument: $1"
      usage >&2
      exit 1
      ;;
  esac
done

if [[ -n "${capture_runner}" && -n "${capture_bin_dir}" ]]; then
  err "Use either --capture-runner or --capture-bin-dir, not both."
  exit 1
fi

if [[ -z "${capture_runner}" && -z "${capture_bin_dir}" ]]; then
  capture_runner=':'
fi

require_tool jq
require_tool unzip

if [[ ! -x "${CAPTURE_SCRIPT}" ]]; then
  err "Missing capture script: ${CAPTURE_SCRIPT}"
  exit 1
fi

manifest_path="$(resolve_path "${manifest_path}")"
output_root="$(resolve_path "${output_root}")"

if [[ ! -f "${manifest_path}" ]]; then
  err "Manifest not found: ${manifest_path}"
  exit 1
fi

if [[ -n "${capture_bin_dir}" ]]; then
  capture_bin_dir="$(resolve_path "${capture_bin_dir}")"
  if [[ ! -d "${capture_bin_dir}" ]]; then
    err "Capture module binary directory not found: ${capture_bin_dir}"
    exit 1
  fi
fi

capture_root="$(mktemp -d "${TMPDIR:-/tmp}/fortran-capture.XXXXXX")"
fixture_meta_file="$(mktemp "${TMPDIR:-/tmp}/fortran-snapshot-meta.XXXXXX")"
trap 'rm -rf "${capture_root}" "${fixture_meta_file}"' EXIT

mkdir -p "${output_root}"

capture_args=(
  --manifest "${manifest_path}"
  --output-root "${capture_root}"
  --all-fixtures
  --allow-missing-entry-files
)

if [[ -n "${capture_bin_dir}" ]]; then
  capture_args+=(--bin-dir "${capture_bin_dir}")
else
  capture_args+=(--runner "${capture_runner}")
fi

log "Running capture for all fixtures"
"${CAPTURE_SCRIPT}" "${capture_args[@]}"

declare -a fixture_ids=()
while IFS= read -r fixture_id; do
  [[ -z "${fixture_id}" ]] && continue
  fixture_ids+=("${fixture_id}")
done < <(jq -r '.fixtures[]?.id' "${manifest_path}")

if [[ "${#fixture_ids[@]}" -eq 0 ]]; then
  err "No fixtures found in manifest."
  exit 1
fi

for fixture_id in "${fixture_ids[@]}"; do
  log "Materializing snapshot ${fixture_id}"

  fixture_json="$(jq -c --arg fixture_id "${fixture_id}" '.fixtures[]? | select(.id == $fixture_id)' "${manifest_path}")"
  if [[ -z "${fixture_json}" ]]; then
    err "Fixture '${fixture_id}' was not found in ${manifest_path}"
    exit 1
  fi

  fixture_root="${output_root}/${fixture_id}"
  baseline_dir="${fixture_root}/baseline"
  checksums_path="${fixture_root}/checksums.sha256"
  snapshot_metadata_path="${fixture_root}/snapshot-metadata.json"
  capture_fixture_root="${capture_root}/${fixture_id}"
  capture_outputs_dir="${capture_fixture_root}/outputs"
  capture_metadata_path="${capture_fixture_root}/metadata.txt"

  rm -rf "${fixture_root}"
  mkdir -p "${baseline_dir}"

  copy_capture_outputs "${capture_outputs_dir}" "${baseline_dir}"

  declare -a source_meta_rows=()
  while IFS= read -r source_row; do
    [[ -z "${source_row}" ]] && continue

    source_kind="$(jq -r '.kind' <<< "${source_row}")"
    source_path="$(jq -r '.path' <<< "${source_row}")"
    resolved_source_path="$(resolve_path "${source_path}")"

    if [[ ! -f "${resolved_source_path}" ]]; then
      err "Fixture '${fixture_id}' baseline source is missing: ${resolved_source_path}"
      exit 1
    fi

    source_sha256="$(sha256_file "${resolved_source_path}")"
    source_meta_rows+=("$(jq -cn \
      --arg kind "${source_kind}" \
      --arg path "${source_path}" \
      --arg sha256 "${source_sha256}" \
      '{kind: $kind, path: $path, sha256: $sha256}')")

    case "${source_kind}" in
      reference_archive)
        extract_archive_into_baseline "${resolved_source_path}" "${baseline_dir}"
        ;;
      reference_file)
        cp "${resolved_source_path}" "${baseline_dir}/$(basename "${source_path}")"
        ;;
      *)
        err "Fixture '${fixture_id}' uses unsupported baseline source kind '${source_kind}'."
        exit 1
        ;;
    esac
  done < <(jq -c '.baselineSources[]?' <<< "${fixture_json}")

  : > "${checksums_path}"
  while IFS= read -r rel_path; do
    file_sha256="$(sha256_file "${baseline_dir}/${rel_path}")"
    printf '%s  %s\n' "${file_sha256}" "${rel_path}" >> "${checksums_path}"
  done < <(cd "${baseline_dir}" && find . -type f | sed 's#^\./##' | LC_ALL=C sort)

  checksummed_file_count="$(wc -l < "${checksums_path}" | tr -d ' ')"
  baseline_status="$(jq -r '.baselineStatus' <<< "${fixture_json}")"
  input_directory="$(jq -r '.inputDirectory' <<< "${fixture_json}")"
  missing_entry_csv=''
  if [[ -f "${capture_metadata_path}" ]]; then
    missing_entry_csv="$(sed -n 's/^missing_entry_files=//p' "${capture_metadata_path}" | head -n 1)"
  fi

  source_meta_json='[]'
  if [[ "${#source_meta_rows[@]}" -gt 0 ]]; then
    source_meta_json="$(printf '%s\n' "${source_meta_rows[@]}" | jq -s '.')"
  fi

  missing_entries_json='[]'
  if [[ -n "${missing_entry_csv}" ]]; then
    missing_entries_json="$(jq -cn --arg csv "${missing_entry_csv}" '$csv | split(",") | map(select(length > 0))')"
  fi

  snapshot_metadata_json="$(jq -cn \
    --arg fixtureId "${fixture_id}" \
    --arg inputDirectory "${input_directory}" \
    --arg baselineStatus "${baseline_status}" \
    --arg checksumsFile "checksums.sha256" \
    --arg generatedAt "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
    --argjson fileCount "${checksummed_file_count}" \
    --argjson baselineSources "${source_meta_json}" \
    --argjson missingEntryFiles "${missing_entries_json}" \
    '{
      fixtureId: $fixtureId,
      inputDirectory: $inputDirectory,
      baselineStatus: $baselineStatus,
      generatedAt: $generatedAt,
      checksumsFile: $checksumsFile,
      checksummedFileCount: $fileCount,
      missingEntryFiles: $missingEntryFiles,
      baselineSources: $baselineSources
    }')"

  printf '%s\n' "${snapshot_metadata_json}" > "${snapshot_metadata_path}"
  printf '%s\n' "${snapshot_metadata_json}" >> "${fixture_meta_file}"
done

snapshot_index_json="$(jq -s 'sort_by(.fixtureId)' "${fixture_meta_file}")"

jq -n \
  --arg manifestPath "${manifest_path}" \
  --arg generatedAt "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" \
  --arg outputRoot "${output_root}" \
  --arg captureMode "$(if [[ -n "${capture_bin_dir}" ]]; then printf 'module_bin_dir'; else printf 'runner'; fi)" \
  --arg captureRunner "${capture_runner}" \
  --arg captureBinDir "${capture_bin_dir}" \
  --argjson fixtures "${snapshot_index_json}" \
  '{
    manifestPath: $manifestPath,
    outputRoot: $outputRoot,
    generatedAt: $generatedAt,
    capture: {
      mode: $captureMode,
      runner: $captureRunner,
      moduleBinDir: $captureBinDir,
      allowMissingEntryFiles: true
    },
    fixtureCount: ($fixtures | length),
    fixtures: $fixtures
  }' > "${output_root}/snapshot-index.json"

log "Generated snapshots for ${#fixture_ids[@]} fixtures at ${output_root}"
