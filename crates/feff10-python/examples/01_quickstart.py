# /// script
# requires-python = ">=3.9"
# dependencies = ["feff10-rs", "matplotlib"]
# ///
"""Quick start: run a Cu K-edge EXAFS calculation in one line.

Usage:
    uv run examples/01_quickstart.py
"""

from pathlib import Path

import feff10
import matplotlib.pyplot as plt

# Define a Cu FCC cluster inline
inp = feff10.FeffInput(
    title=["Cu K-edge EXAFS — quick start example"],
    edge="K",
    s02=1.0,
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

# Run the calculation
result = feff10.run(inp, "./work_quickstart")

# Print per-stage timings
for sr in result.stages:
    print(f"  {sr.stage.executable_name:>10}: {sr.duration_secs:.3f}s")
print(f"  {'total':>10}: {result.total_duration_secs:.3f}s")

# Read the output spectrum
xmu = feff10.FeffTable.from_file(f"{result.work_dir}/xmu.dat")
print(f"\nOutput: {xmu.nrows} data points, {xmu.ncols} columns")
print(xmu)

# Plot Energy (col 0) vs XMU/mu(E) (col 3) and save figure
energy = xmu[0]
xmu_values = xmu[3]

fig, ax = plt.subplots(figsize=(8, 5))
ax.plot(energy, xmu_values, lw=1.8)
ax.set_xlabel("Energy (eV)")
ax.set_ylabel("xmu (mu(E))")
ax.set_title("Cu K-edge xmu.dat")
ax.grid(alpha=0.25)
fig.tight_layout()

plot_path = Path(result.work_dir) / "xmu_energy_vs_xmu.png"
fig.savefig(plot_path, dpi=150)
print(f"\nSaved plot: {plot_path}")
