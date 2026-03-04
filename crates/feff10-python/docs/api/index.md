# API Reference

## Classes

### Input

| Class | Description |
|---|---|
| [`FeffInput`](input.md#feffinput) | FEFF input file representation |
| [`Potential`](input.md#potential) | Scattering potential definition |
| [`Atom`](input.md#atom) | Atomic position in the cluster |

### Pipeline

| Class | Description |
|---|---|
| [`FeffConfig`](pipeline.md#feffconfig) | Calculation configuration |
| [`Stage`](pipeline.md#stage) | Pipeline stage enum (18 stages) |
| [`FeffPipeline`](pipeline.md#feffpipeline) | Pipeline executor |
| [`PipelineResult`](pipeline.md#pipelineresult) | Pipeline execution result |
| [`StageResult`](pipeline.md#stageresult) | Per-stage timing result |
| [`StageProgress`](pipeline.md#stageprogress) | Progress callback data |

### Output

| Class | Description |
|---|---|
| [`FeffTable`](output.md#fefftable) | Parsed xmu.dat output file |

## Exception Hierarchy

```
FeffError (base)
├── FeffIOError        — file I/O errors
├── FeffParseError     — input/output parsing errors
├── FeffPipelineError  — pipeline execution errors
└── FeffConfigError    — configuration validation errors
```

All exceptions inherit from `FeffError`, which inherits from Python's `Exception`.
