# /// script
# requires-python = ">=3.9"
# dependencies = ["feff10-rs"]
# ///
"""Parse and inspect xmu.dat output files.

Usage:
    uv run examples/04_parse_output.py

Note: Run 01_quickstart.py first to generate the output files.
"""

import os
import sys

import feff10

xmu_path = "./work_quickstart/xmu.dat"
if not os.path.exists(xmu_path):
    print(f"Output not found at {xmu_path}")
    print("Run 01_quickstart.py first to generate it.")
    sys.exit(1)

xmu = feff10.XmuDat.from_file(xmu_path)

# Basic info
print(f"Columns: {xmu.ncols}")
print(f"Rows:    {xmu.nrows}")
print(f"Header:  {len(xmu.header)} lines")
for line in xmu.header[:3]:
    print(f"  {line}")

# Access columns
energy = xmu[0]  # first column (omega/energy)
mu = xmu[3]      # fourth column (chi or mu, depending on calc type)
print(f"\nEnergy range: {min(energy):.2f} to {max(energy):.2f}")
print(f"Column 3 range: {min(mu):.6f} to {max(mu):.6f}")

# Iterate over all columns
print(f"\nAll columns:")
for i, col in enumerate(xmu):
    print(f"  col[{i}]: {len(col)} points, range [{min(col):.4f}, {max(col):.4f}]")

# Preview
print(f"\nFirst 5 rows:")
print(xmu)
