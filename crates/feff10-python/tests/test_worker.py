"""Exercise the installed package's worker on every supported platform."""

import math
from pathlib import Path

import pytest

import feff10


def copper_input():
    source = Path(__file__).parents[2] / "feff10-cli/examples/bundled/exafs-cu.inp"
    return source.read_text().replace("RPATH 5.5", "RPATH 5.2").replace(
        "PRINT 0 0 0 0 0 0", "PRINT 0 0 0 0 0 3"
    )


def assert_copper_paths(work_dir, result):
    paths = sorted(work_dir.glob("feff[0-9][0-9][0-9][0-9].dat"))
    assert len(paths) == 14
    geometry = next(
        line for line in paths[0].read_text().splitlines() if "nleg, deg, reff" in line
    )
    assert [float(v) for v in geometry.split()[:3]] == pytest.approx(
        [2, 12, 2.5527], abs=0.0001
    )
    for path in paths:
        lines = path.read_text().splitlines()
        start = next(i for i, line in enumerate(lines) if "real[2*phc]" in line)
        rows = [[float(v) for v in line.split()] for line in lines[start + 1:] if line.strip()]
        assert len(rows) > 50
        assert all(len(row) == 7 and all(math.isfinite(v) for v in row) for row in rows)
        assert any(row[2] > 0 for row in rows)
    chi = result.read_chi(strict=True)
    assert chi.nrows > 50
    assert all(math.isfinite(v) for column in chi.columns for v in column)
    assert any(abs(v) > 1e-8 for v in chi.column(1))


def test_python_workers_survive_failure_and_repeat_calculations(tmp_path):
    content = copper_input()
    inp = feff10.FeffInput.parse(content)
    config = feff10.FeffConfig(
        str(tmp_path / "missing phase"), inp,
        stages=[feff10.Stage.RDINP, feff10.Stage.GENFMT],
    )
    with pytest.raises(feff10.FeffPipelineError, match="genfmt"):
        feff10.FeffPipeline(config).run()

    # All public entry points must launch the same installed Python worker.
    # Spaces and Unicode also exercise native working-directory arguments.
    for index in range(3):
        work_dir = tmp_path / f"Cu calculation 銅 {index}"
        if index == 0:
            result = feff10.run(content, str(work_dir))
        else:
            pipeline = feff10.FeffPipeline(feff10.FeffConfig(str(work_dir), inp))
            if index == 1:
                result = pipeline.run()
            else:
                events = []
                result = pipeline.run_with_progress(
                    lambda stage, progress: events.append(progress.kind)
                )
                assert events == ["starting", "finished"] * len(result.stages)
        assert_copper_paths(work_dir, result)
