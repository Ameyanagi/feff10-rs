#!/usr/bin/env python3
"""Generate dependency-aware Fortran-to-Rust migration gap artifacts.

Outputs:
  - gap/fortran-to-rust-file-gap.csv
  - gap/fortran-to-rust-gap-report.md
"""

from __future__ import annotations

import argparse
import csv
import json
import re
import sys
from collections import Counter, defaultdict, deque
from datetime import datetime
from pathlib import Path
from typing import Iterable


MODULE_DEF_RE = re.compile(r"^\s*module\s+([a-z][a-z0-9_]*)\b", re.IGNORECASE)
SUBROUTINE_DEF_RE = re.compile(
    r"^\s*(?:(?:recursive|pure|elemental|impure|module)\s+)*subroutine\s+([a-z][a-z0-9_]*)\b",
    re.IGNORECASE,
)
FUNCTION_DEF_RE = re.compile(
    r"^\s*(?:(?:recursive|pure|elemental|impure|module)\s+)*"
    r"(?:(?:[a-z][a-z0-9_]*(?:\s*\([^)]+\))?)\s+)*"
    r"function\s+([a-z][a-z0-9_]*)\b",
    re.IGNORECASE,
)
USE_RE = re.compile(
    r"^\s*use\s*(?:,\s*(?:intrinsic|non_intrinsic)\s*)?(?:::\s*)?([a-z][a-z0-9_]*)\b",
    re.IGNORECASE,
)
CALL_RE = re.compile(r"\bcall\s+([a-z][a-z0-9_]*)\b", re.IGNORECASE)

FORTRAN_SUFFIXES = (".f", ".f90", ".for", ".f95", ".f03", ".f08")

MODULE_TO_DIRS = {
    "RDINP": ("RDINP",),
    "POT": ("POT",),
    "PATH": ("PATH",),
    "FMS": ("FMS",),
    "XSPH": ("XSPH",),
    "BAND": ("BAND",),
    "LDOS": ("LDOS",),
    "RIXS": ("RIXS",),
    "CRPA": ("CRPA",),
    "COMPTON": ("COMPTON",),
    "DEBYE": ("DEBYE", "FF2X"),
    "DMDW": ("DMDW",),
    "SCREEN": ("SCREEN",),
    "SELF": ("SELF", "SFCONV"),
    "EELS": ("EELS",),
    "FULLSPECTRUM": ("FULLSPECTRUM",),
}

MODULE_TO_OUTPUT_NAME = {
    "RDINP": "rdinp",
    "POT": "pot",
    "PATH": "path",
    "FMS": "fms",
    "XSPH": "xsph",
    "BAND": "band",
    "LDOS": "ldos",
    "RIXS": "rixs",
    "CRPA": "crpa",
    "COMPTON": "compton",
    "DEBYE": "debye",
    "DMDW": "dmdw",
    "SCREEN": "screen",
    "SELF": "self",
    "EELS": "eels",
    "FULLSPECTRUM": "fullspectrum",
}


class FileData:
    def __init__(self, rel_path: str, fortran_dir: str) -> None:
        self.rel_path = rel_path
        self.fortran_dir = fortran_dir
        self.defined_modules: set[str] = set()
        self.defined_routines: set[str] = set()
        self.uses: set[str] = set()
        self.calls: set[str] = set()
        self.unresolved_targets: set[str] = set()
        self.resolved_deps: set[str] = set()
        self.origin_modules: set[str] = set()
        self.classification = "out_of_scope"


def parse_args() -> argparse.Namespace:
    repo_root = Path(__file__).resolve().parents[2]
    parser = argparse.ArgumentParser(
        description="Generate dependency-aware Fortran-to-Rust gap artifacts."
    )
    parser.add_argument(
        "--manifest",
        default="tasks/golden-fixture-manifest.json",
        help="Fixture manifest path (default: tasks/golden-fixture-manifest.json)",
    )
    parser.add_argument(
        "--fortran-root",
        default="feff10/src",
        help="Fortran source root to scan (default: feff10/src)",
    )
    parser.add_argument(
        "--csv-output",
        default="gap/fortran-to-rust-file-gap.csv",
        help="CSV output path (default: gap/fortran-to-rust-file-gap.csv)",
    )
    parser.add_argument(
        "--report-output",
        default="gap/fortran-to-rust-gap-report.md",
        help="Markdown output path (default: gap/fortran-to-rust-gap-report.md)",
    )
    parser.add_argument(
        "--repo-root",
        default=str(repo_root),
        help="Repository root for path resolution (default: inferred from script location)",
    )
    return parser.parse_args()


def resolve_path(repo_root: Path, raw_path: str) -> Path:
    path = Path(raw_path)
    if path.is_absolute():
        return path
    return (repo_root / path).resolve()


def strip_comments(line: str) -> str:
    if "!" not in line:
        return line
    return line.split("!", 1)[0]


def list_fortran_files(root: Path) -> list[Path]:
    files: list[Path] = []
    for path in root.rglob("*"):
        if path.is_file() and path.suffix.lower() in FORTRAN_SUFFIXES:
            files.append(path)
    return sorted(files)


def parse_fortran_file(path: Path) -> tuple[set[str], set[str], set[str], set[str]]:
    defined_modules: set[str] = set()
    defined_routines: set[str] = set()
    uses: set[str] = set()
    calls: set[str] = set()

    with path.open("r", encoding="utf-8", errors="ignore") as handle:
        for raw_line in handle:
            line = strip_comments(raw_line).strip()
            if not line:
                continue

            lower = line.lower()

            module_match = MODULE_DEF_RE.match(lower)
            if module_match:
                module_name = module_match.group(1)
                if module_name not in {"procedure", "subroutine", "function"}:
                    defined_modules.add(module_name)

            sub_match = SUBROUTINE_DEF_RE.match(lower)
            if sub_match:
                defined_routines.add(sub_match.group(1))

            func_match = FUNCTION_DEF_RE.match(lower)
            if func_match:
                defined_routines.add(func_match.group(1))

            use_match = USE_RE.match(lower)
            if use_match:
                uses.add(use_match.group(1))

            for call_match in CALL_RE.finditer(lower):
                calls.add(call_match.group(1))

    return defined_modules, defined_routines, uses, calls


def load_runtime_modules(manifest_path: Path) -> list[str]:
    with manifest_path.open("r", encoding="utf-8") as handle:
        manifest = json.load(handle)
    modules = manifest.get("inScopeModules", [])
    if not isinstance(modules, list) or not modules:
        raise ValueError(
            f"manifest at {manifest_path} is missing a non-empty inScopeModules array"
        )
    return [str(module).upper() for module in modules]


def build_dir_to_modules(runtime_modules: Iterable[str]) -> dict[str, set[str]]:
    mapping: dict[str, set[str]] = defaultdict(set)
    for module in runtime_modules:
        dirs = MODULE_TO_DIRS.get(module, (module,))
        for directory in dirs:
            mapping[directory.upper()].add(module)
    return mapping


def primary_runtime_module_name(modules: set[str]) -> str:
    if not modules:
        return "none"
    if len(modules) > 1:
        return "multiple"
    only = next(iter(modules))
    return MODULE_TO_OUTPUT_NAME.get(only, only.lower())


def generate_csv(csv_path: Path, records: list[FileData]) -> None:
    csv_path.parent.mkdir(parents=True, exist_ok=True)
    with csv_path.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.writer(handle)
        writer.writerow(
            [
                "fortran_path",
                "fortran_dir",
                "classification",
                "primary_runtime_module",
                "resolved_dependency_count",
                "unresolved_target_count",
            ]
        )
        for record in records:
            writer.writerow(
                [
                    record.rel_path,
                    record.fortran_dir,
                    record.classification,
                    primary_runtime_module_name(record.origin_modules),
                    len(record.resolved_deps),
                    len(record.unresolved_targets),
                ]
            )


def generate_markdown_report(
    report_path: Path,
    records: list[FileData],
    manifest_rel: str,
    fortran_glob_display: str,
    csv_rel: str,
) -> None:
    report_path.parent.mkdir(parents=True, exist_ok=True)

    classification_totals = Counter(record.classification for record in records)
    directory_totals: dict[str, Counter] = defaultdict(Counter)
    unresolved_symbol_counter: Counter[str] = Counter()

    for record in records:
        directory_totals[record.fortran_dir][record.classification] += 1
        for symbol in record.unresolved_targets:
            unresolved_symbol_counter[symbol] += 1

    support_dirs_sorted = sorted(
        (
            (directory, counts.get("runtime_support_dependency", 0))
            for directory, counts in directory_totals.items()
        ),
        key=lambda item: (-item[1], item[0]),
    )
    support_dirs_sorted = [item for item in support_dirs_sorted if item[1] > 0]

    unresolved_files = sum(1 for record in records if record.unresolved_targets)
    unresolved_total = sum(len(record.unresolved_targets) for record in records)

    lines: list[str] = []
    lines.append("# Fortran-to-Rust Gap Report")
    lines.append("")
    lines.append(f"Generated: {datetime.now().astimezone().strftime('%Y-%m-%d %H:%M:%S %Z')}")
    lines.append("")
    lines.append("## Scope")
    lines.append(f"- Source scanned: `{fortran_glob_display}`")
    lines.append(
        f"- Runtime-module contract source: `{manifest_rel}` (`inScopeModules`)"
    )
    lines.append(f"- Full file inventory: `{csv_rel}`")
    lines.append("- Unresolved policy: conservative external (no expansion through unresolved targets)")
    lines.append("")
    lines.append("## Method")
    lines.append("- Parse each Fortran file for `module`, `subroutine`, `function` definitions.")
    lines.append("- Parse `use` and `call` references and resolve to local definitions when possible.")
    lines.append("- Seed graph traversal with runtime-owned files from v1 module directories.")
    lines.append("- Classify files as `runtime_owned`, `runtime_support_dependency`, or `out_of_scope`.")
    lines.append("")
    lines.append("## Totals")
    lines.append(f"- Total Fortran source files found: **{len(records)}**")
    lines.append(
        f"- `runtime_owned`: **{classification_totals.get('runtime_owned', 0)}**"
    )
    lines.append(
        f"- `runtime_support_dependency`: **{classification_totals.get('runtime_support_dependency', 0)}**"
    )
    lines.append(
        f"- `out_of_scope`: **{classification_totals.get('out_of_scope', 0)}**"
    )
    lines.append("")
    lines.append("## Directory Coverage")
    lines.append(
        "| Fortran Dir | File Count | runtime_owned | runtime_support_dependency | out_of_scope |"
    )
    lines.append("| --- | ---: | ---: | ---: | ---: |")
    for directory in sorted(directory_totals):
        counts = directory_totals[directory]
        total = sum(counts.values())
        lines.append(
            f"| {directory} | {total} | {counts.get('runtime_owned', 0)} | "
            f"{counts.get('runtime_support_dependency', 0)} | {counts.get('out_of_scope', 0)} |"
        )
    lines.append("")
    lines.append("## Top Runtime Support Directories")
    if support_dirs_sorted:
        lines.append("| Fortran Dir | runtime_support_dependency files |")
        lines.append("| --- | ---: |")
        for directory, count in support_dirs_sorted[:15]:
            lines.append(f"| {directory} | {count} |")
    else:
        lines.append("No runtime support dependency files were detected outside runtime-owned directories.")
    lines.append("")
    lines.append("## Unresolved Target Diagnostics")
    lines.append(f"- Files with unresolved targets: **{unresolved_files}**")
    lines.append(f"- Total unresolved target entries (unique per file): **{unresolved_total}**")
    if unresolved_symbol_counter:
        lines.append("- Top unresolved targets by file count:")
        lines.append("")
        lines.append("| Target | Files |")
        lines.append("| --- | ---: |")
        for symbol, count in unresolved_symbol_counter.most_common(20):
            lines.append(f"| `{symbol}` | {count} |")
    else:
        lines.append("- No unresolved targets detected.")
    lines.append("")
    lines.append("## Notes")
    lines.append("- This is static parsing, not a full Fortran semantic/AST analysis.")
    lines.append("- `call`/`use` symbols may resolve to multiple files; reachability uses unioned edges.")
    lines.append("- `feff10/` is a local reference checkout (not tracked in git).")

    report_path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> int:
    args = parse_args()
    repo_root = Path(args.repo_root).resolve()
    manifest_path = resolve_path(repo_root, args.manifest)
    fortran_root = resolve_path(repo_root, args.fortran_root)
    csv_output = resolve_path(repo_root, args.csv_output)
    report_output = resolve_path(repo_root, args.report_output)

    if not manifest_path.is_file():
        print(f"[generate-gap-report] ERROR: manifest not found: {manifest_path}", file=sys.stderr)
        return 1
    if not fortran_root.is_dir():
        print(f"[generate-gap-report] ERROR: fortran root not found: {fortran_root}", file=sys.stderr)
        return 1

    runtime_modules = load_runtime_modules(manifest_path)
    dir_to_modules = build_dir_to_modules(runtime_modules)

    files = list_fortran_files(fortran_root)
    if not files:
        print(f"[generate-gap-report] ERROR: no Fortran files found under {fortran_root}", file=sys.stderr)
        return 1

    file_data: dict[str, FileData] = {}
    module_defs: dict[str, set[str]] = defaultdict(set)
    routine_defs: dict[str, set[str]] = defaultdict(set)

    for path in files:
        rel = path.relative_to(repo_root).as_posix()
        dir_name = path.parent.name.upper()
        record = FileData(rel_path=rel, fortran_dir=dir_name)
        (
            record.defined_modules,
            record.defined_routines,
            record.uses,
            record.calls,
        ) = parse_fortran_file(path)
        file_data[rel] = record

        for symbol in record.defined_modules:
            module_defs[symbol].add(rel)
        for symbol in record.defined_routines:
            routine_defs[symbol].add(rel)

    adjacency: dict[str, set[str]] = defaultdict(set)

    for rel, record in file_data.items():
        for used_module in sorted(record.uses):
            targets = module_defs.get(used_module, set())
            targets = {target for target in targets if target != rel}
            if targets:
                adjacency[rel].update(targets)
            else:
                record.unresolved_targets.add(f"use::{used_module}")

        for called_routine in sorted(record.calls):
            targets = routine_defs.get(called_routine, set())
            targets = {target for target in targets if target != rel}
            if targets:
                adjacency[rel].update(targets)
            else:
                record.unresolved_targets.add(f"call::{called_routine}")

        record.resolved_deps = set(sorted(adjacency.get(rel, set())))

    runtime_owned: set[str] = set()
    for rel, record in file_data.items():
        modules = dir_to_modules.get(record.fortran_dir, set())
        if modules:
            runtime_owned.add(rel)
            record.origin_modules.update(modules)

    queue: deque[str] = deque(sorted(runtime_owned))
    queued = set(queue)
    while queue:
        current = queue.popleft()
        queued.discard(current)
        current_origins = file_data[current].origin_modules
        if not current_origins:
            continue
        for dep in sorted(adjacency.get(current, set())):
            target_origins = file_data[dep].origin_modules
            before = len(target_origins)
            target_origins.update(current_origins)
            if len(target_origins) > before and dep not in queued:
                queue.append(dep)
                queued.add(dep)

    records = [file_data[path] for path in sorted(file_data)]
    for record in records:
        if record.rel_path in runtime_owned:
            record.classification = "runtime_owned"
        elif record.origin_modules:
            record.classification = "runtime_support_dependency"
        else:
            record.classification = "out_of_scope"

    generate_csv(csv_output, records)

    manifest_rel = manifest_path.relative_to(repo_root).as_posix()
    csv_rel = csv_output.relative_to(repo_root).as_posix()
    fortran_glob_display = f"{fortran_root.relative_to(repo_root).as_posix()}/**/*.f*"
    generate_markdown_report(
        report_output,
        records,
        manifest_rel=manifest_rel,
        fortran_glob_display=fortran_glob_display,
        csv_rel=csv_rel,
    )

    report_rel = report_output.relative_to(repo_root).as_posix()
    print(f"[generate-gap-report] Wrote {csv_rel}")
    print(f"[generate-gap-report] Wrote {report_rel}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
