# /// script
# requires-python = ">=3.9"
# dependencies = ["feff10-rs"]
# ///
"""Build a feff.inp programmatically, modify it, and write to file.

Usage:
    uv run examples/07_build_input.py
"""

import feff10

# Build a Cu K-edge input from scratch
inp = feff10.FeffInput(
    title=["Cu K-edge EXAFS", "Built programmatically"],
    edge="K",
    s02=1.0,
    control=(1, 1, 1, 1, 1, 1),
    potentials=[
        feff10.Potential(ipot=0, z=29, tag="Cu"),
        feff10.Potential(ipot=1, z=29, tag="Cu"),
    ],
    atoms=[
        feff10.Atom(x=0.0, y=0.0, z=0.0, ipot=0, tag="Cu"),
        feff10.Atom(x=0.0, y=1.805, z=1.805, ipot=1, tag="Cu"),
        feff10.Atom(x=1.805, y=0.0, z=1.805, ipot=1, tag="Cu"),
        feff10.Atom(x=1.805, y=1.805, z=0.0, ipot=1, tag="Cu"),
    ],
    other_cards=["EXAFS 20.0", "RPATH 5.5"],
)

# Inspect
print(f"Edge:       {inp.edge}")
print(f"S02:        {inp.s02}")
print(f"Potentials: {inp.num_potentials}")
print(f"Atoms:      {inp.num_atoms}")
print(f"CONTROL:    {inp.control}")

for pot in inp.potentials:
    print(f"  {pot}")

for atom in inp.atoms:
    print(f"  {atom}")

# Modify
inp.edge = "L3"
inp.s02 = 0.85
inp.control = (1, 1, 1, 0, 0, 0)  # only run first 3 stage groups

# Validate and write
inp.validate()
inp.write_to_file("modified_feff.inp")
print(f"\nWrote modified_feff.inp (edge={inp.edge}, S02={inp.s02})")

# Read it back
inp2 = feff10.FeffInput.from_file("modified_feff.inp")
print(f"Re-read: edge={inp2.edge}, S02={inp2.s02}, atoms={inp2.num_atoms}")

# Clean up
import os
os.remove("modified_feff.inp")
