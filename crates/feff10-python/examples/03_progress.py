# /// script
# requires-python = ">=3.9"
# dependencies = ["feff10-rs"]
# ///
"""Run a calculation with a progress callback.

Usage:
    uv run examples/03_progress.py
"""

import feff10

inp = feff10.FeffInput(
    title=["Cu K-edge with progress tracking"],
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


def on_progress(stage, progress):
    if progress.kind == "starting":
        print(f"  [{stage.executable_name:>10}] running...", end="", flush=True)
    else:
        print(f" done ({progress.duration_secs:.2f}s)")


# Validate first, then run with progress (the full pipeline API)
inp.validate()
config = feff10.FeffConfig("./work_progress", inp)
result = feff10.FeffPipeline(config).run_with_progress(on_progress)

print(f"\nTotal: {result.total_duration_secs:.2f}s")
print(f"Output directory: {result.work_dir}")
