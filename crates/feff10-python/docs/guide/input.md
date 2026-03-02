# Working with Input Files

The `feff10` package can parse, create, modify, and write FEFF input files (`feff.inp`).

## Parsing an Input File

```python
import feff10

# From a file path
inp = feff10.FeffInput.from_file("feff.inp")

# From a string
content = open("feff.inp").read()
inp = feff10.FeffInput.parse(content)
```

Use strict parsing to catch formatting errors:

```python
# Raises FeffParseError on malformed CONTROL, POTENTIALS, ATOMS, etc.
inp = feff10.FeffInput.parse_strict(content)
inp = feff10.FeffInput.from_file_strict("feff.inp")
```

## Inspecting the Input

```python
inp = feff10.FeffInput.from_file("feff.inp")

print(inp.edge)          # "K", "L3", etc.
print(inp.s02)           # amplitude reduction factor
print(inp.num_atoms)     # number of atoms
print(inp.num_potentials)  # number of unique potentials

# CONTROL and PRINT flags (6-element lists)
print(inp.control)       # e.g. [1, 1, 1, 1, 1, 1]
print(inp.print_flags)   # e.g. [0, 0, 0, 0, 0, 0]

# Other cards (EXAFS, RPATH, etc.)
for card in inp.other_cards:
    print(card)
```

## Working with Potentials

```python
# Access potentials
pots = inp.potentials  # returns a list (cloned — cache for repeated use)
for pot in pots:
    print(f"ipot={pot.ipot}, Z={pot.z}, tag={pot.tag}")

# Create a potential
pot = feff10.Potential(ipot=0, z=29, tag="Cu")
pot = feff10.Potential(ipot=1, z=26, tag="Fe", l_scmt=2, l_fms=3, stoich=1.5)
```

## Working with Atoms

```python
# Access atoms
atoms = inp.atoms  # returns a list (cloned — cache for repeated use)
for atom in atoms:
    print(f"({atom.x}, {atom.y}, {atom.z}) ipot={atom.ipot} {atom.tag} d={atom.distance}")

# Create an atom
atom = feff10.Atom(x=1.805, y=1.805, z=0.0, ipot=1, tag="Cu", distance=2.5527)
```

## Creating an Input from Scratch

```python
inp = feff10.FeffInput(
    title=["Cu crystal EXAFS"],
    edge="K",
    s02=1.0,
    potentials=[
        feff10.Potential(ipot=0, z=29, tag="Cu"),
        feff10.Potential(ipot=1, z=29, tag="Cu"),
    ],
    atoms=[
        feff10.Atom(x=0.0, y=0.0, z=0.0, ipot=0, tag="Cu"),
        feff10.Atom(x=0.0, y=1.805, z=1.805, ipot=1, tag="Cu"),
    ],
    other_cards=["EXAFS 20.0", "RPATH 5.5"],
)
```

## Modifying an Input

All properties have setters:

```python
inp = feff10.FeffInput.from_file("feff.inp")
inp.edge = "L3"
inp.s02 = 0.85
inp.title = ["Modified calculation"]
inp.control = [1, 1, 1, 1, 0, 0]  # disable last two stage groups
```

## Writing to File

```python
# Write to a file
inp.write_to_file("output.inp")

# Get as a string
text = inp.to_string()
print(text)

# Roundtrip: parse → modify → write
inp = feff10.FeffInput.from_file("feff.inp")
inp.edge = "L3"
inp.write_to_file("feff_L3.inp")
```
