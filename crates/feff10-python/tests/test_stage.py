import pytest

import feff10


def test_all_stages():
    stages = feff10.Stage.all()
    assert len(stages) == 18


def test_default_pipeline():
    pipeline = feff10.Stage.default_pipeline()
    assert len(pipeline) == 18
    assert pipeline == feff10.Stage.all()


def test_from_name():
    stage = feff10.Stage.from_name("pot")
    assert stage == feff10.Stage.POT


def test_from_name_case_insensitive():
    stage = feff10.Stage.from_name("PoT")
    assert stage == feff10.Stage.POT


def test_from_name_invalid():
    with pytest.raises(ValueError):
        feff10.Stage.from_name("badstage")


def test_executable_name():
    assert feff10.Stage.RDINP.executable_name == "rdinp"
    assert feff10.Stage.POT.executable_name == "pot"
    assert feff10.Stage.FF2X.executable_name == "ff2x"


def test_control_index():
    assert feff10.Stage.RDINP.control_index == 0
    assert feff10.Stage.POT.control_index == 1
    assert feff10.Stage.XSPH.control_index == 2
    assert feff10.Stage.FMS.control_index == 3
    assert feff10.Stage.GENFMT.control_index == 4
    assert feff10.Stage.FF2X.control_index == 5


def test_stage_equality():
    assert feff10.Stage.POT == feff10.Stage.POT
    assert feff10.Stage.POT != feff10.Stage.FMS


def test_stage_repr():
    r = repr(feff10.Stage.POT)
    assert "POT" in r


def test_stage_hashable():
    s = {feff10.Stage.POT, feff10.Stage.FMS, feff10.Stage.POT}
    assert len(s) == 2
