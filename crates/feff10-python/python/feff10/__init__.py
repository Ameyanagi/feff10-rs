"""Python bindings for FEFF10 X-ray absorption spectroscopy calculations.

This package provides a Pythonic interface to FEFF10, a real-space
multiple-scattering code for ab initio calculations of X-ray absorption
spectra (EXAFS, XANES), electronic structure, and related properties.

Example:
    ```python
    import feff10

    # Simple one-liner:
    result = feff10.run("feff.inp", "./work")

    # Or with full control:
    inp = feff10.FeffInput.from_file("feff.inp")
    config = feff10.FeffConfig("./work", inp)
    result = feff10.FeffPipeline(config).run()
    ```
"""

from feff10._feff10 import (
    # Input types
    Potential,
    Atom,
    FeffInput,
    # Configuration
    FeffConfig,
    # Stage enum
    Stage,
    # Pipeline
    FeffPipeline,
    PipelineResult,
    StageResult,
    StageProgress,
    # Output
    XmuDat,
    # Exceptions
    FeffError,
    FeffIOError,
    FeffParseError,
    FeffPipelineError,
    FeffConfigError,
    # Convenience functions
    run,
    validate,
)

__all__ = [
    "run",
    "validate",
    "Potential",
    "Atom",
    "FeffInput",
    "FeffConfig",
    "Stage",
    "FeffPipeline",
    "PipelineResult",
    "StageResult",
    "StageProgress",
    "XmuDat",
    "FeffError",
    "FeffIOError",
    "FeffParseError",
    "FeffPipelineError",
    "FeffConfigError",
]

from importlib.metadata import version as _version

__version__ = _version("feff10-rs")
