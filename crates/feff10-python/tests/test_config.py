import pytest

import feff10


def test_config_basic(sample_feff_inp, tmp_path):
    inp = feff10.FeffInput.parse(sample_feff_inp)
    config = feff10.FeffConfig(str(tmp_path), inp)
    assert config.work_dir == str(tmp_path)
    assert len(config.stages) > 0
    assert config.stage_timeout is None


def test_config_with_stages(sample_feff_inp, tmp_path):
    inp = feff10.FeffInput.parse(sample_feff_inp)
    stages = [feff10.Stage.RDINP, feff10.Stage.POT]
    config = feff10.FeffConfig(str(tmp_path), inp, stages=stages)
    assert len(config.stages) == 2
    assert config.stages[0] == feff10.Stage.RDINP
    assert config.stages[1] == feff10.Stage.POT


def test_config_with_timeout(sample_feff_inp, tmp_path):
    inp = feff10.FeffInput.parse(sample_feff_inp)
    config = feff10.FeffConfig(str(tmp_path), inp, stage_timeout=30.0)
    assert config.stage_timeout == 30.0


def test_config_from_file(sample_feff_inp_file, tmp_path):
    config = feff10.FeffConfig.from_file(str(tmp_path), sample_feff_inp_file)
    assert len(config.stages) > 0


def test_config_from_file_not_found(tmp_path):
    with pytest.raises(feff10.FeffIOError):
        feff10.FeffConfig.from_file(str(tmp_path), "/nonexistent/feff.inp")


def test_config_derives_stages_from_control(sample_feff_inp, tmp_path):
    inp = feff10.FeffInput.parse(sample_feff_inp)
    # All CONTROL flags are 1, so all stages should be present
    config = feff10.FeffConfig(str(tmp_path), inp)
    assert len(config.stages) == 18


def test_config_repr(sample_feff_inp, tmp_path):
    inp = feff10.FeffInput.parse(sample_feff_inp)
    config = feff10.FeffConfig(str(tmp_path), inp)
    r = repr(config)
    assert "FeffConfig" in r
    assert str(tmp_path) in r


def test_config_stage_timeout_value(sample_feff_inp, tmp_path):
    inp = feff10.FeffInput.parse(sample_feff_inp)
    config = feff10.FeffConfig(str(tmp_path), inp, stage_timeout=30.5)
    assert config.stage_timeout == 30.5


def test_config_stage_timeout_default_none(sample_feff_inp, tmp_path):
    inp = feff10.FeffInput.parse(sample_feff_inp)
    config = feff10.FeffConfig(str(tmp_path), inp)
    assert config.stage_timeout is None
