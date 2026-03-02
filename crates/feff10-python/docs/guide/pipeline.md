# Running Calculations

The pipeline module handles configuring and executing FEFF10 calculations.

## Basic Usage

```python
import feff10

# Parse input and configure
inp = feff10.FeffInput.from_file("feff.inp")
config = feff10.FeffConfig("./work", inp)

# Run the full pipeline
pipeline = feff10.FeffPipeline(config)
result = pipeline.run()

print(f"Completed {len(result.stages)} stages in {result.total_duration_secs:.1f}s")
print(f"Output in: {result.work_dir}")
```

## Configuration Options

### From an Input Object

```python
config = feff10.FeffConfig(
    work_dir="./work",
    input=inp,
    stages=None,           # derive from CONTROL card (default)
    stage_timeout=60.0,    # timeout per stage in seconds (Unix only)
)
```

### From a File Path

```python
config = feff10.FeffConfig.from_file(
    work_dir="./work",
    input_file="feff.inp",
)
```

### Running Specific Stages

```python
config = feff10.FeffConfig(
    "./work", inp,
    stages=[feff10.Stage.RDINP, feff10.Stage.POT, feff10.Stage.XSPH],
)
```

## Pipeline Stages

FEFF10 has 18 stages, each a separate computational step:

```python
# List all stages
for stage in feff10.Stage.all():
    print(f"{stage.executable_name} (control index {stage.control_index})")

# Default pipeline (commonly used stages)
default = feff10.Stage.default_pipeline()

# Look up a stage by name
pot = feff10.Stage.from_name("pot")
```

## Progress Callbacks

Monitor calculation progress with a callback function:

```python
def on_progress(stage, progress):
    if progress.kind == "starting":
        print(f"  Running {stage.executable_name}...", end="", flush=True)
    else:
        print(f" done ({progress.duration_secs:.2f}s)")

result = pipeline.run_with_progress(on_progress)
```

The callback receives two arguments:

- `stage` (`Stage`) — which stage is running
- `progress` (`StageProgress`) — either `"starting"` or `"finished"` with duration

If the callback raises an exception, it is captured and re-raised after the current stage completes.

## GIL Behavior

Both `run()` and `run_with_progress()` release the Python GIL during FEFF stage execution. This allows other Python threads to run concurrently while FEFF computes.

For `run_with_progress()`, the GIL is briefly re-acquired only to invoke the callback between stages.

## Error Handling

```python
try:
    result = pipeline.run()
except feff10.FeffPipelineError as e:
    print(f"Pipeline failed: {e}")
except feff10.FeffConfigError as e:
    print(f"Configuration error: {e}")
```

## Inspecting Results

```python
result = pipeline.run()

# Per-stage timing
for sr in result.stages:
    print(f"{sr.stage.executable_name}: {sr.duration_secs:.3f}s")

# Total time
print(f"Total: {result.total_duration_secs:.3f}s")

# Output directory
print(f"Results in: {result.work_dir}")
```
