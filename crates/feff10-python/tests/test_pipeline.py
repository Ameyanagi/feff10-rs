import os

import pytest

from conftest import has_submodule

import feff10


@pytest.mark.skipif(not has_submodule(), reason="feff10 submodule not available")
def test_pipeline_basic(tmp_path):
    """Integration test: run a fast FEFF calculation (XANES/BN)."""
    repo_root = os.path.dirname(
        os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
    )
    inp_path = os.path.join(repo_root, "feff10", "examples", "XANES", "BN", "feff.inp")
    inp = feff10.FeffInput.from_file(inp_path)
    work_dir = str(tmp_path / "work")
    config = feff10.FeffConfig(work_dir, inp)
    pipeline = feff10.FeffPipeline(config)
    result = pipeline.run()
    assert len(result.stages) > 0
    assert result.total_duration_secs > 0
    assert result.work_dir == work_dir
    outputs = result.outputs()
    assert len(outputs.files) > 0
    xmu = result.read_xmu()
    assert xmu.ncols >= 4


@pytest.mark.skipif(not has_submodule(), reason="feff10 submodule not available")
def test_pipeline_with_progress(tmp_path):
    """Integration test: run with progress callback."""
    repo_root = os.path.dirname(
        os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
    )
    inp_path = os.path.join(repo_root, "feff10", "examples", "XANES", "BN", "feff.inp")
    inp = feff10.FeffInput.from_file(inp_path)
    work_dir = str(tmp_path / "work")
    config = feff10.FeffConfig(work_dir, inp)
    pipeline = feff10.FeffPipeline(config)

    events = []

    def on_progress(stage, progress):
        events.append((stage, progress.kind))

    result = pipeline.run_with_progress(on_progress)
    assert len(result.stages) > 0
    # Each stage should produce two events: starting and finished
    assert len(events) == len(result.stages) * 2


def test_pipeline_result_properties():
    """Test PipelineResult and StageResult repr (without running FEFF)."""
    # We can't easily create a PipelineResult without running FEFF,
    # so just test what we can about the types.
    assert hasattr(feff10, "PipelineResult")
    assert hasattr(feff10, "StageResult")
    assert hasattr(feff10, "StageProgress")
    assert hasattr(feff10, "FeffPipeline")


@pytest.mark.skipif(not has_submodule(), reason="feff10 submodule not available")
def test_callback_error_propagation(tmp_path):
    """If the progress callback raises, the error should propagate."""
    repo_root = os.path.dirname(
        os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
    )
    inp_path = os.path.join(repo_root, "feff10", "examples", "XANES", "BN", "feff.inp")
    inp = feff10.FeffInput.from_file(inp_path)
    work_dir = str(tmp_path / "work")
    config = feff10.FeffConfig(work_dir, inp)
    pipeline = feff10.FeffPipeline(config)

    def bad_callback(stage, progress):
        raise ValueError("callback error on purpose")

    with pytest.raises(ValueError, match="callback error on purpose"):
        pipeline.run_with_progress(bad_callback)
