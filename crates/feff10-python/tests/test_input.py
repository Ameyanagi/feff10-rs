import feff10


def test_parse(sample_feff_inp):
    inp = feff10.FeffInput.parse(sample_feff_inp)
    assert inp.edge == "K"
    assert inp.s02 == 1.0
    assert len(inp.potentials) == 2
    assert len(inp.atoms) == 4
    assert list(inp.control) == [1, 1, 1, 1, 1, 1]


def test_parse_strict(sample_feff_inp):
    inp = feff10.FeffInput.parse_strict(sample_feff_inp)
    assert inp.edge == "K"
    assert len(inp.potentials) == 2


def test_from_file(sample_feff_inp_file):
    inp = feff10.FeffInput.from_file(sample_feff_inp_file)
    assert inp.edge == "K"
    assert len(inp.atoms) == 4


def test_to_string_roundtrip(sample_feff_inp):
    inp = feff10.FeffInput.parse(sample_feff_inp)
    text = inp.to_string()
    reparsed = feff10.FeffInput.parse(text)
    assert len(reparsed.potentials) == len(inp.potentials)
    assert len(reparsed.atoms) == len(inp.atoms)
    assert reparsed.edge == inp.edge


def test_write_to_file(sample_feff_inp, tmp_path):
    inp = feff10.FeffInput.parse(sample_feff_inp)
    out_path = str(tmp_path / "output.inp")
    inp.write_to_file(out_path)
    reparsed = feff10.FeffInput.from_file(out_path)
    assert len(reparsed.atoms) == 4


def test_potential_properties():
    pot = feff10.Potential(ipot=0, z=29, tag="Cu")
    assert pot.ipot == 0
    assert pot.z == 29
    assert pot.tag == "Cu"
    assert pot.l_scmt is None
    assert repr(pot) == "Potential(ipot=0, z=29, tag='Cu')"


def test_potential_optional_fields():
    pot = feff10.Potential(ipot=1, z=26, tag="Fe", l_scmt=2, l_fms=3, stoich=1.5)
    assert pot.l_scmt == 2
    assert pot.l_fms == 3
    assert pot.stoich == 1.5


def test_potential_equality():
    a = feff10.Potential(ipot=0, z=29, tag="Cu")
    b = feff10.Potential(ipot=0, z=29, tag="Cu")
    c = feff10.Potential(ipot=1, z=26, tag="Fe")
    assert a == b
    assert a != c


def test_potential_equality_includes_optional_fields():
    a = feff10.Potential(ipot=0, z=29, tag="Cu", l_scmt=2)
    b = feff10.Potential(ipot=0, z=29, tag="Cu", l_scmt=3)
    c = feff10.Potential(ipot=0, z=29, tag="Cu", l_scmt=2)
    assert a != b
    assert a == c


def test_atom_properties():
    atom = feff10.Atom(x=1.0, y=2.0, z=3.0, ipot=0, tag="Cu")
    assert atom.x == 1.0
    assert atom.y == 2.0
    assert atom.z == 3.0
    assert atom.ipot == 0
    assert atom.tag == "Cu"
    assert atom.distance == 0.0


def test_atom_with_distance():
    atom = feff10.Atom(x=1.805, y=1.805, z=0.0, ipot=1, tag="Cu", distance=2.5527)
    assert abs(atom.distance - 2.5527) < 1e-4


def test_feff_input_constructor():
    inp = feff10.FeffInput(
        title=["Test"],
        edge="K",
        s02=0.9,
        potentials=[feff10.Potential(ipot=0, z=29, tag="Cu")],
        atoms=[feff10.Atom(x=0.0, y=0.0, z=0.0, ipot=0, tag="Cu")],
    )
    assert inp.edge == "K"
    assert inp.s02 == 0.9
    assert len(inp.potentials) == 1
    assert len(inp.atoms) == 1


def test_feff_input_default_constructor():
    inp = feff10.FeffInput()
    assert inp.edge is None
    assert inp.s02 is None
    assert len(inp.potentials) == 0
    assert len(inp.atoms) == 0
    assert list(inp.control) == [1, 1, 1, 1, 1, 1]
    assert list(inp.print_flags) == [0, 0, 0, 0, 0, 0]


def test_feff_input_setters():
    inp = feff10.FeffInput()
    inp.edge = "L3"
    inp.s02 = 0.85
    inp.title = ["New title"]
    assert inp.edge == "L3"
    assert inp.s02 == 0.85
    assert inp.title == ["New title"]


def test_feff_input_repr(sample_feff_inp):
    inp = feff10.FeffInput.parse(sample_feff_inp)
    r = repr(inp)
    assert "FeffInput" in r
    assert "potentials=2" in r
    assert "atoms=4" in r


def test_feff_input_str(sample_feff_inp):
    inp = feff10.FeffInput.parse(sample_feff_inp)
    s = str(inp)
    assert "TITLE" in s
    assert "POTENTIALS" in s
    assert "ATOMS" in s


def test_other_cards_preserved(sample_feff_inp):
    inp = feff10.FeffInput.parse(sample_feff_inp)
    cards_upper = [c.upper() for c in inp.other_cards]
    assert any("EXAFS" in c for c in cards_upper)
    assert any("RPATH" in c for c in cards_upper)


def test_parse_error_on_bad_file():
    import pytest

    with pytest.raises(feff10.FeffIOError):
        feff10.FeffInput.from_file("/nonexistent/path/feff.inp")


def test_parse_strict_error():
    import pytest

    bad = """\
TITLE test
CONTROL 1 1 1
PRINT 0 0 0 0 0 0
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
"""
    with pytest.raises(feff10.FeffParseError):
        feff10.FeffInput.parse_strict(bad)


def test_atom_equality():
    a = feff10.Atom(x=1.0, y=2.0, z=3.0, ipot=0, tag="Cu")
    b = feff10.Atom(x=1.0, y=2.0, z=3.0, ipot=0, tag="Cu")
    c = feff10.Atom(x=0.0, y=0.0, z=0.0, ipot=1, tag="Fe")
    assert a == b
    assert a != c


def test_atom_equality_includes_distance():
    a = feff10.Atom(x=1.0, y=2.0, z=3.0, ipot=0, tag="Cu", distance=2.5)
    b = feff10.Atom(x=1.0, y=2.0, z=3.0, ipot=0, tag="Cu", distance=3.5)
    c = feff10.Atom(x=1.0, y=2.0, z=3.0, ipot=0, tag="Cu", distance=2.5)
    assert a != b
    assert a == c


def test_atom_str():
    atom = feff10.Atom(x=1.805, y=1.805, z=0.0, ipot=1, tag="Cu")
    s = str(atom)
    assert "1.80500" in s
    assert "Cu" in s


def test_potential_str():
    pot = feff10.Potential(ipot=0, z=29, tag="Cu")
    s = str(pot)
    assert "29" in s
    assert "Cu" in s


def test_num_potentials_and_atoms(sample_feff_inp):
    inp = feff10.FeffInput.parse(sample_feff_inp)
    assert inp.num_potentials == 2
    assert inp.num_atoms == 4


def test_control_array_wrong_size():
    import pytest

    with pytest.raises((TypeError, ValueError)):
        feff10.FeffInput(control=[1, 2, 3])
