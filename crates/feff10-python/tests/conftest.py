import os

import pytest

SAMPLE_FEFF_INP = """\
TITLE Cu crystal EXAFS
EDGE K
S02 1.0

CONTROL 1 1 1 1 1 1
PRINT 0 0 0 0 0 0

EXAFS 20.0
RPATH 5.5

POTENTIALS
0 29 Cu
1 29 Cu

ATOMS
  0.00000    0.00000    0.00000  0    Cu            0.00000
  0.00000    1.80500    1.80500  1    Cu            2.55270
  1.80500    0.00000    1.80500  1    Cu            2.55270
  1.80500    1.80500    0.00000  1    Cu            2.55270
END
"""

SAMPLE_XMU_DAT = """\
# FEFF10 output
# col_0: omega
# col_1: mu
# col_2: mu0
# col_3: chi
1.0 10.0 8.0 0.25
2.0 20.0 16.0 0.25
3.0 30.0 24.0 0.25
4.0 40.0 32.0 0.25
5.0 50.0 40.0 0.25
"""


@pytest.fixture
def sample_feff_inp():
    return SAMPLE_FEFF_INP


@pytest.fixture
def sample_xmu_dat():
    return SAMPLE_XMU_DAT


@pytest.fixture
def sample_feff_inp_file(tmp_path):
    path = tmp_path / "feff.inp"
    path.write_text(SAMPLE_FEFF_INP)
    return str(path)


@pytest.fixture
def sample_xmu_dat_file(tmp_path):
    path = tmp_path / "xmu.dat"
    path.write_text(SAMPLE_XMU_DAT)
    return str(path)


def has_submodule():
    """Check if the feff10 git submodule is available."""
    repo_root = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
    return os.path.exists(os.path.join(repo_root, "feff10", "examples", "EXAFS", "Cu", "feff.inp"))
