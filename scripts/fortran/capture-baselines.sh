#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

log() {
  printf '[capture-baselines] %s\n' "$*"
}

err() {
  printf '[capture-baselines] ERROR: %s\n' "$*" >&2
}

usage() {
  cat <<'USAGE'
Usage:
  scripts/fortran/capture-baselines.sh [options]

Options:
  --manifest <path>       Fixture manifest JSON path (default: tasks/golden-fixture-manifest.json)
  --output-root <path>    Output root for captures (default: artifacts/fortran-baselines)
  --runner <command>      Command to run per fixture in the fixture output directory
  --bin-dir <path>        Directory containing Fortran module executables (alternative to --runner)
  --fixture <id>          Fixture ID to run (repeatable)
  --fixtures <id,id,...>  Comma-separated fixture IDs to run
  --all-fixtures          Run all fixtures in the manifest
  --help                  Show this help text

Notes:
- If no fixture IDs are provided, fixtures with baselineStatus=requires_fortran_capture are selected.
- Any fixture failure causes a non-zero exit code.
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

copy_top_level_inputs() {
  local fixture_input_dir="$1"
  local fixture_inputs_dir="$2"
  local file_path
  local base_name

  while IFS= read -r -d '' file_path; do
    base_name="$(basename "${file_path}")"
    case "${base_name}" in
      REFERENCE.zip|reference*|Reference*)
        continue
        ;;
    esac
    cp "${file_path}" "${fixture_inputs_dir}/${base_name}"
  done < <(find "${fixture_input_dir}" -maxdepth 1 -type f -print0)
}

materialize_entry_file() {
  local fixture_input_dir="$1"
  local entry_file="$2"
  local fixture_inputs_dir="$3"
  local fixture_outputs_dir="$4"
  shift 4
  local archive_sources=("$@")

  local entry_basename
  local input_target
  local output_target
  local archive_path
  local resolved_archive_path

  entry_basename="$(basename "${entry_file}")"
  input_target="${fixture_inputs_dir}/${entry_basename}"
  output_target="${fixture_outputs_dir}/${entry_basename}"

  if [[ -f "${fixture_input_dir}/${entry_file}" ]]; then
    cp "${fixture_input_dir}/${entry_file}" "${input_target}"
    cp "${fixture_input_dir}/${entry_file}" "${output_target}"
    return
  fi

  if [[ -f "${fixture_input_dir}/${entry_basename}" ]]; then
    cp "${fixture_input_dir}/${entry_basename}" "${input_target}"
    cp "${fixture_input_dir}/${entry_basename}" "${output_target}"
    return
  fi

  if [[ "${#archive_sources[@]}" -eq 0 ]]; then
    err "Fixture entry '${entry_file}' is missing and no reference_archive source is available."
    return 1
  fi

  require_tool unzip

  for archive_path in "${archive_sources[@]}"; do
    resolved_archive_path="$(resolve_path "${archive_path}")"
    if [[ ! -f "${resolved_archive_path}" ]]; then
      continue
    fi

    if unzip -Z1 "${resolved_archive_path}" | grep -Fx -- "${entry_file}" >/dev/null; then
      unzip -p "${resolved_archive_path}" "${entry_file}" > "${input_target}"
      cp "${input_target}" "${output_target}"
      return
    fi

    if unzip -Z1 "${resolved_archive_path}" | grep -Fx -- "REFERENCE/${entry_basename}" >/dev/null; then
      unzip -p "${resolved_archive_path}" "REFERENCE/${entry_basename}" > "${input_target}"
      cp "${input_target}" "${output_target}"
      return
    fi

    if unzip -Z1 "${resolved_archive_path}" | grep -Fx -- "${entry_basename}" >/dev/null; then
      unzip -p "${resolved_archive_path}" "${entry_basename}" > "${input_target}"
      cp "${input_target}" "${output_target}"
      return
    fi
  done

  err "Unable to materialize fixture entry '${entry_file}' for '${fixture_input_dir}'."
  return 1
}

run_with_runner_command() {
  local fixture_outputs_dir="$1"
  local runner_command="$2"
  local fixture_log_file="$3"

  (
    set +e
    cd "${fixture_outputs_dir}" || exit 1
    bash -lc "${runner_command}"
    exit $?
  ) >> "${fixture_log_file}" 2>&1
}

run_with_module_binaries() {
  local fixture_outputs_dir="$1"
  local module_bin_dir="$2"
  local fixture_log_file="$3"

  local module_specs=(
    'rdinp:feff.inp'
    'dmdw:dmdw.inp'
    'atomic:global.inp'
    'pot:pot.inp'
    'ldos:ldos.inp'
    'screen:screen.inp'
    'opconsat:opcons.inp'
    'xsph:xsph.inp'
    'fms:fms.inp'
    'path:paths.inp'
    'mkgtr:paths.dat'
    'genfmt:genfmt.inp'
    'ff2x:ff2x.inp'
    'sfconv:sfconv.inp'
    'compton:compton.inp'
    'eels:eels.inp'
    'rhorrp:density.inp'
    'band:band.inp'
    'crpa:crpa.inp'
    'rixs:rixs.inp'
    'fullspectrum:fullspectrum.inp'
  )

  local spec
  local module
  local trigger_file
  local executable_path
  local status
  local ran_modules=0

  for spec in "${module_specs[@]}"; do
    module="${spec%%:*}"
    trigger_file="${spec#*:}"

    if [[ ! -f "${fixture_outputs_dir}/${trigger_file}" ]]; then
      continue
    fi

    executable_path="${module_bin_dir}/${module}"
    if [[ ! -x "${executable_path}" ]]; then
      echo "Missing executable for module '${module}': ${executable_path}" >> "${fixture_log_file}"
      return 1
    fi

    echo "[module:${module}] trigger=${trigger_file}" >> "${fixture_log_file}"

    (
      set +e
      cd "${fixture_outputs_dir}" || exit 1
      "${executable_path}"
      exit $?
    ) >> "${fixture_log_file}" 2>&1
    status=$?

    if [[ "${status}" -ne 0 ]]; then
      echo "Module '${module}' failed with exit code ${status}." >> "${fixture_log_file}"
      return "${status}"
    fi

    ran_modules=$((ran_modules + 1))
  done

  if [[ "${ran_modules}" -eq 0 ]]; then
    echo "No module binaries were executed. Check fixture inputs and module sequence." >> "${fixture_log_file}"
    return 1
  fi
}

manifest_path='tasks/golden-fixture-manifest.json'
output_root='artifacts/fortran-baselines'
runner_command=''
module_bin_dir=''
all_fixtures=0
declare -a requested_fixtures=()

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
    --runner)
      runner_command="$2"
      shift 2
      ;;
    --bin-dir)
      module_bin_dir="$2"
      shift 2
      ;;
    --fixture)
      requested_fixtures+=("$2")
      shift 2
      ;;
    --fixtures)
      IFS=',' read -r -a fixture_list <<< "$2"
      requested_fixtures+=("${fixture_list[@]}")
      shift 2
      ;;
    --all-fixtures)
      all_fixtures=1
      shift
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

if [[ -n "${runner_command}" && -n "${module_bin_dir}" ]]; then
  err "Use either --runner or --bin-dir, not both."
  exit 1
fi

if [[ -z "${runner_command}" && -z "${module_bin_dir}" ]]; then
  err "One execution mode is required: --runner or --bin-dir."
  exit 1
fi

require_tool jq

manifest_path="$(resolve_path "${manifest_path}")"
output_root="$(resolve_path "${output_root}")"

if [[ ! -f "${manifest_path}" ]]; then
  err "Manifest not found: ${manifest_path}"
  exit 1
fi

if [[ -n "${module_bin_dir}" ]]; then
  module_bin_dir="$(resolve_path "${module_bin_dir}")"
  if [[ ! -d "${module_bin_dir}" ]]; then
    err "Module binary directory not found: ${module_bin_dir}"
    exit 1
  fi
fi

declare -a fixture_ids=()

if [[ "${#requested_fixtures[@]}" -gt 0 ]]; then
  fixture_ids=("${requested_fixtures[@]}")
elif [[ "${all_fixtures}" -eq 1 ]]; then
  while IFS= read -r fixture_id; do
    [[ -z "${fixture_id}" ]] && continue
    fixture_ids+=("${fixture_id}")
  done < <(jq -r '.fixtures[]?.id' "${manifest_path}")
else
  while IFS= read -r fixture_id; do
    [[ -z "${fixture_id}" ]] && continue
    fixture_ids+=("${fixture_id}")
  done < <(jq -r '.fixtures[]? | select(.baselineStatus == "requires_fortran_capture") | .id' "${manifest_path}")
fi

if [[ "${#fixture_ids[@]}" -eq 0 ]]; then
  err "No fixtures were selected."
  exit 1
fi

mkdir -p "${output_root}"

selected_file="${output_root}/selected-fixtures.txt"
: > "${selected_file}"
for fixture_id in "${fixture_ids[@]}"; do
  printf '%s\n' "${fixture_id}" >> "${selected_file}"
done

success_count=0
failure_count=0

for fixture_id in "${fixture_ids[@]}"; do
  log "Capturing fixture ${fixture_id}"

  fixture_json="$(jq -c --arg fixture_id "${fixture_id}" '.fixtures[]? | select(.id == $fixture_id)' "${manifest_path}")"
  if [[ -z "${fixture_json}" ]]; then
    err "Fixture '${fixture_id}' was not found in ${manifest_path}."
    failure_count=$((failure_count + 1))
    continue
  fi

  input_directory="$(jq -r '.inputDirectory' <<< "${fixture_json}")"
  baseline_status="$(jq -r '.baselineStatus' <<< "${fixture_json}")"
  resolved_input_directory="$(resolve_path "${input_directory}")"

  if [[ ! -d "${resolved_input_directory}" ]]; then
    err "Fixture input directory does not exist for '${fixture_id}': ${resolved_input_directory}"
    failure_count=$((failure_count + 1))
    continue
  fi

  fixture_root="${output_root}/${fixture_id}"
  fixture_inputs_dir="${fixture_root}/inputs"
  fixture_outputs_dir="${fixture_root}/outputs"
  fixture_logs_dir="${fixture_root}/logs"
  fixture_log_file="${fixture_logs_dir}/runner.log"

  rm -rf "${fixture_root}"
  mkdir -p "${fixture_inputs_dir}" "${fixture_outputs_dir}" "${fixture_logs_dir}"

  copy_top_level_inputs "${resolved_input_directory}" "${fixture_inputs_dir}"

  find "${fixture_inputs_dir}" -maxdepth 1 -type f -exec cp {} "${fixture_outputs_dir}/" \;

  declare -a entry_files=()
  while IFS= read -r entry_file; do
    [[ -z "${entry_file}" ]] && continue
    entry_files+=("${entry_file}")
  done < <(jq -r '.entryFiles[]?' <<< "${fixture_json}")

  declare -a archive_sources=()
  while IFS= read -r archive_source; do
    [[ -z "${archive_source}" ]] && continue
    archive_sources+=("${archive_source}")
  done < <(jq -r '.baselineSources[]? | select(.kind == "reference_archive") | .path' <<< "${fixture_json}")

  entry_failure=0
  for entry_file in "${entry_files[@]}"; do
    if ! materialize_entry_file "${resolved_input_directory}" "${entry_file}" "${fixture_inputs_dir}" "${fixture_outputs_dir}" "${archive_sources[@]}"; then
      entry_failure=1
      break
    fi
  done

  if [[ "${entry_failure}" -ne 0 ]]; then
    failure_count=$((failure_count + 1))
    continue
  fi

  {
    printf 'fixture_id=%s\n' "${fixture_id}"
    printf 'baseline_status=%s\n' "${baseline_status}"
    printf 'input_directory=%s\n' "${resolved_input_directory}"
    if [[ -n "${runner_command}" ]]; then
      printf 'run_mode=runner\n'
    else
      printf 'run_mode=module_bin_dir\n'
      printf 'module_bin_dir=%s\n' "${module_bin_dir}"
    fi
  } > "${fixture_root}/metadata.txt"

  if [[ -n "${runner_command}" ]]; then
    if ! run_with_runner_command "${fixture_outputs_dir}" "${runner_command}" "${fixture_log_file}"; then
      err "Fixture '${fixture_id}' failed while running --runner command."
      failure_count=$((failure_count + 1))
      continue
    fi
  else
    if ! run_with_module_binaries "${fixture_outputs_dir}" "${module_bin_dir}" "${fixture_log_file}"; then
      err "Fixture '${fixture_id}' failed while running module binaries."
      failure_count=$((failure_count + 1))
      continue
    fi
  fi

  success_count=$((success_count + 1))
  log "Captured fixture ${fixture_id}"
done

log "Summary: success=${success_count}, failure=${failure_count}, selected=${#fixture_ids[@]}"

if [[ "${failure_count}" -ne 0 ]]; then
  exit 1
fi
