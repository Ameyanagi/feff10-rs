# /// script
# requires-python = ">=3.9"
# dependencies = ["feff10-rs"]
# ///
"""Validate a FEFF input before running.

Usage:
    uv run examples/02_validate.py
"""

import feff10

# --- Valid input ---
valid = feff10.FeffInput(
    title=["Valid Cu input"],
    edge="K",
    potentials=[
        feff10.Potential(ipot=0, z=29, tag="Cu"),
        feff10.Potential(ipot=1, z=29, tag="Cu"),
    ],
    atoms=[
        feff10.Atom(x=0.0, y=0.0, z=0.0, ipot=0, tag="Cu"),
        feff10.Atom(x=1.805, y=1.805, z=0.0, ipot=1, tag="Cu"),
    ],
)

valid.validate()
print("Valid input: OK")

# --- Invalid: missing absorber ---
bad_no_absorber = feff10.FeffInput(
    potentials=[
        feff10.Potential(ipot=1, z=29, tag="Cu"),  # no ipot=0!
    ],
    atoms=[
        feff10.Atom(x=0.0, y=0.0, z=0.0, ipot=1, tag="Cu"),
    ],
)

try:
    bad_no_absorber.validate()
except feff10.FeffConfigError as e:
    print(f"\nMissing absorber caught:\n  {e}")

# --- Invalid: atom references undefined potential ---
bad_ref = feff10.FeffInput(
    potentials=[
        feff10.Potential(ipot=0, z=29, tag="Cu"),
    ],
    atoms=[
        feff10.Atom(x=0.0, y=0.0, z=0.0, ipot=0, tag="Cu"),
        feff10.Atom(x=1.805, y=0.0, z=0.0, ipot=5, tag="X"),  # ipot=5 not defined
    ],
)

try:
    bad_ref.validate()
except feff10.FeffConfigError as e:
    print(f"\nUndefined ipot caught:\n  {e}")

# --- Module-level validate() from file ---
import tempfile, os

path = os.path.join(tempfile.mkdtemp(), "feff.inp")
valid.write_to_file(path)
feff10.validate(path)
print(f"\nFile validation: OK ({path})")
