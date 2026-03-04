# /// script
# requires-python = ">=3.9"
# dependencies = ["feff10-rs[pandas]"]
# ///
"""Export FEFF output to a pandas DataFrame.

Usage:
    uv run examples/06_pandas.py

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

xmu = feff10.FeffTable.from_file(xmu_path)
df = xmu.to_dataframe()

print(f"DataFrame shape: {df.shape}")
print(f"\n{df.describe()}")
print(f"\nFirst 5 rows:\n{df.head()}")
