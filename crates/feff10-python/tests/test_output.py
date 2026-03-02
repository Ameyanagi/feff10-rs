import math

import pytest

import feff10


def test_parse(sample_xmu_dat):
    xmu = feff10.XmuDat.parse(sample_xmu_dat)
    assert xmu.ncols == 4
    assert xmu.nrows == 5
    assert len(xmu.header) == 5


def test_parse_strict(sample_xmu_dat):
    xmu = feff10.XmuDat.parse_strict(sample_xmu_dat)
    assert xmu.ncols == 4


def test_from_file(sample_xmu_dat_file):
    xmu = feff10.XmuDat.from_file(sample_xmu_dat_file)
    assert xmu.ncols == 4
    assert xmu.nrows == 5


def test_columns(sample_xmu_dat):
    xmu = feff10.XmuDat.parse(sample_xmu_dat)
    cols = xmu.columns
    assert len(cols) == 4
    assert cols[0] == [1.0, 2.0, 3.0, 4.0, 5.0]


def test_column_method(sample_xmu_dat):
    xmu = feff10.XmuDat.parse(sample_xmu_dat)
    col0 = xmu.column(0)
    assert col0 == [1.0, 2.0, 3.0, 4.0, 5.0]


def test_column_out_of_range(sample_xmu_dat):
    xmu = feff10.XmuDat.parse(sample_xmu_dat)
    with pytest.raises(IndexError):
        xmu.column(10)


def test_r_squared_identical(sample_xmu_dat):
    xmu = feff10.XmuDat.parse(sample_xmu_dat)
    rsq = xmu.r_squared(xmu, col_x=0, col_y=1)
    assert rsq < 1e-10


def test_r_squared_different():
    c1 = "1.0 10.0\n2.0 20.0\n3.0 30.0\n4.0 40.0\n5.0 50.0\n"
    c2 = "1.0 15.0\n2.0 25.0\n3.0 35.0\n4.0 45.0\n5.0 55.0\n"
    xmu1 = feff10.XmuDat.parse(c1)
    xmu2 = feff10.XmuDat.parse(c2)
    rsq = xmu1.r_squared(xmu2, col_x=0, col_y=1)
    assert rsq > 0.0
    assert math.isfinite(rsq)


def test_r_squared_no_overlap():
    c1 = "1.0 10.0\n2.0 20.0\n3.0 30.0\n"
    c2 = "5.0 50.0\n6.0 60.0\n7.0 70.0\n"
    xmu1 = feff10.XmuDat.parse(c1)
    xmu2 = feff10.XmuDat.parse(c2)
    rsq = xmu1.r_squared(xmu2, col_x=0, col_y=1)
    assert math.isnan(rsq)


def test_parse_empty():
    xmu = feff10.XmuDat.parse("")
    assert xmu.ncols == 0
    assert xmu.nrows == 0
    assert len(xmu.header) == 0


def test_parse_header_only():
    xmu = feff10.XmuDat.parse("# header 1\n# header 2\n")
    assert len(xmu.header) == 2
    assert xmu.ncols == 0


def test_repr(sample_xmu_dat):
    xmu = feff10.XmuDat.parse(sample_xmu_dat)
    r = repr(xmu)
    assert "XmuDat" in r
    assert "columns=4" in r
    assert "rows=5" in r


def test_len(sample_xmu_dat):
    xmu = feff10.XmuDat.parse(sample_xmu_dat)
    assert len(xmu) == 5


def test_from_file_not_found():
    with pytest.raises(feff10.FeffIOError):
        feff10.XmuDat.from_file("/nonexistent/xmu.dat")


def test_parse_strict_rejects_ragged():
    bad = "1.0 2.0 3.0\n4.0 5.0\n"
    with pytest.raises(feff10.FeffParseError):
        feff10.XmuDat.parse_strict(bad)


def test_parse_strict_rejects_invalid_token():
    bad = "1.0 2.0\n3.0 abc\n"
    with pytest.raises(feff10.FeffParseError):
        feff10.XmuDat.parse_strict(bad)


def test_getitem(sample_xmu_dat):
    xmu = feff10.XmuDat.parse(sample_xmu_dat)
    assert xmu[0] == [1.0, 2.0, 3.0, 4.0, 5.0]
    assert xmu[3] == [0.25, 0.25, 0.25, 0.25, 0.25]


def test_getitem_negative(sample_xmu_dat):
    xmu = feff10.XmuDat.parse(sample_xmu_dat)
    assert xmu[-1] == xmu[3]
    assert xmu[-4] == xmu[0]


def test_getitem_out_of_range(sample_xmu_dat):
    xmu = feff10.XmuDat.parse(sample_xmu_dat)
    with pytest.raises(IndexError):
        xmu[10]
    with pytest.raises(IndexError):
        xmu[-5]


def test_iter(sample_xmu_dat):
    xmu = feff10.XmuDat.parse(sample_xmu_dat)
    cols = list(xmu)
    assert len(cols) == 4
    assert cols[0] == [1.0, 2.0, 3.0, 4.0, 5.0]


def test_str(sample_xmu_dat):
    xmu = feff10.XmuDat.parse(sample_xmu_dat)
    s = str(xmu)
    assert "4 columns" in s
    assert "5 rows" in s


def test_str_empty():
    xmu = feff10.XmuDat.parse("")
    assert "empty" in str(xmu)


def test_to_dataframe_import_error():
    """to_dataframe raises ImportError if pandas is not installed."""
    xmu = feff10.XmuDat.parse("1.0 2.0\n3.0 4.0\n")
    try:
        import pandas  # noqa: F401

        # If pandas is installed, to_dataframe should work
        df = xmu.to_dataframe()
        assert df.shape == (2, 2)
    except ImportError:
        with pytest.raises(ImportError):
            xmu.to_dataframe()
