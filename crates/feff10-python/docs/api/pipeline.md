# Pipeline

## FeffConfig

Configuration for a FEFF calculation.

### Constructors

```python
FeffConfig(
    work_dir: str,
    input: FeffInput,
    stages: Optional[list[Stage]] = None,
    stage_timeout: Optional[float] = None,
)
```

| Parameter | Description |
|---|---|
| `work_dir` | Working directory for the calculation |
| `input` | Parsed `FeffInput` object |
| `stages` | Stages to run (default: derived from CONTROL card) |
| `stage_timeout` | Timeout per stage in seconds (Unix only) |

### Static Methods

| Method | Returns | Description |
|---|---|---|
| `from_file(work_dir, input_file, stages=None, stage_timeout=None)` | `FeffConfig` | Create from a feff.inp file path |

### Properties

| Property | Type | Description |
|---|---|---|
| `work_dir` | `str` | Working directory |
| `stages` | `list[Stage]` | Stages to run |
| `stage_timeout` | `Optional[float]` | Timeout per stage (seconds) |

---

## Stage

Enum of the 18 FEFF10 pipeline stages.

### Variants

| Stage | Index | Description |
|---|---|---|
| `Stage.RDINP` | 0 | Read input |
| `Stage.DMDW` | 1 | Debye-Waller factors |
| `Stage.ATOMIC` | 2 | Atomic potentials |
| `Stage.POT` | 3 | Muffin-tin potentials |
| `Stage.LDOS` | 4 | Local density of states |
| `Stage.SCREEN` | 5 | Core-hole screening |
| `Stage.CRPA` | 6 | Constrained RPA |
| `Stage.OPCONSAT` | 7 | Optical constants |
| `Stage.XSPH` | 8 | Phase shifts |
| `Stage.FMS` | 9 | Full multiple scattering |
| `Stage.MKGTR` | 10 | Green's function |
| `Stage.PATH` | 11 | Path finder |
| `Stage.GENFMT` | 12 | Generate F-matrix |
| `Stage.FF2X` | 13 | Convert to chi(k) |
| `Stage.SFCONV` | 14 | Spectral convolution |
| `Stage.COMPTON` | 15 | Compton scattering |
| `Stage.EELS` | 16 | Electron energy loss |
| `Stage.RHORRP` | 17 | Charge density |

### Static Methods

| Method | Returns | Description |
|---|---|---|
| `Stage.all()` | `list[Stage]` | All 18 stages in pipeline order |
| `Stage.default_pipeline()` | `list[Stage]` | Default pipeline stages |
| `Stage.from_name(name: str)` | `Stage` | Look up stage by name (case-insensitive) |

### Properties

| Property | Type | Description |
|---|---|---|
| `executable_name` | `str` | Stage executable name (e.g. `"rdinp"`) |
| `control_index` | `int` | Index in the CONTROL array |

Supports `==`, `hash()`, and `repr()`.

---

## FeffPipeline

Executes a configured FEFF calculation.

### Constructor

```python
FeffPipeline(config: FeffConfig)
```

### Methods

#### `run() -> PipelineResult`

Run the full pipeline. Releases the Python GIL during computation so other threads can run concurrently.

#### `run_with_progress(callback) -> PipelineResult`

Run with a progress callback.

```python
def callback(stage: Stage, progress: StageProgress) -> None: ...
```

The GIL is released during each FEFF stage and re-acquired only to invoke the callback. If the callback raises an exception, it is re-raised after the current stage completes.

---

## PipelineResult

Result of a pipeline execution.

### Properties

| Property | Type | Description |
|---|---|---|
| `stages` | `list[StageResult]` | Per-stage results |
| `work_dir` | `str` | Output directory |
| `total_duration_secs` | `float` | Total wall-clock time (seconds) |

---

## StageResult

Result of a single stage execution.

### Properties

| Property | Type | Description |
|---|---|---|
| `stage` | `Stage` | Which stage ran |
| `duration_secs` | `float` | Wall-clock time (seconds) |

---

## StageProgress

Progress information passed to callbacks.

### Properties

| Property | Type | Description |
|---|---|---|
| `kind` | `str` | `"starting"` or `"finished"` |
| `duration_secs` | `Optional[float]` | Duration in seconds (only when `kind="finished"`) |
