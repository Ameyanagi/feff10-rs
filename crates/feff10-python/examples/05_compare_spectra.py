# /// script
# requires-python = ">=3.9"
# dependencies = ["feff10-rs"]
# ///
"""Compare two FEFF calculations using R-squared metric.

Runs the same Cu cluster with two different S02 values and compares
the resulting spectra.

Usage:
    uv run examples/05_compare_spectra.py
"""

import feff10

ATOMS = [
    feff10.Atom(x=0.0, y=0.0, z=0.0, ipot=0, tag="Cu"),
    feff10.Atom(x=0.0, y=1.805, z=1.805, ipot=1, tag="Cu"),
    feff10.Atom(x=1.805, y=0.0, z=1.805, ipot=1, tag="Cu"),
    feff10.Atom(x=1.805, y=1.805, z=0.0, ipot=1, tag="Cu"),
]

POTS = [
    feff10.Potential(ipot=0, z=29, tag="Cu"),
    feff10.Potential(ipot=1, z=29, tag="Cu"),
]

# Run with S02 = 1.0
inp1 = feff10.FeffInput(
    title=["Cu K-edge, S02=1.0"],
    edge="K",
    s02=1.0,
    potentials=POTS,
    atoms=ATOMS,
    other_cards=["EXAFS 20.0"],
)
print("Running with S02=1.0...")
r1 = feff10.run(inp1, "./work_compare_1")

# Run with S02 = 0.85
inp2 = feff10.FeffInput(
    title=["Cu K-edge, S02=0.85"],
    edge="K",
    s02=0.85,
    potentials=POTS,
    atoms=ATOMS,
    other_cards=["EXAFS 20.0"],
)
print("Running with S02=0.85...")
r2 = feff10.run(inp2, "./work_compare_2")

# Compare the output spectra
xmu1 = feff10.XmuDat.from_file(f"{r1.work_dir}/xmu.dat")
xmu2 = feff10.XmuDat.from_file(f"{r2.work_dir}/xmu.dat")

rsq = xmu1.r_squared(xmu2, col_x=0, col_y=3)
print(f"\nR-squared (col_x=0, col_y=3): {rsq * 100:.4f}%")
print("(Lower is more similar; 0% = identical)")
