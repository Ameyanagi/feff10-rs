"""Type stubs for the feff10 Python package."""

from typing import Callable, Iterator, Optional

class FeffError(Exception): ...
class FeffIOError(FeffError): ...
class FeffParseError(FeffError): ...
class FeffPipelineError(FeffError): ...
class FeffConfigError(FeffError): ...

class Potential:
    ipot: int
    z: int
    tag: str
    l_scmt: Optional[int]
    l_fms: Optional[int]
    stoich: Optional[float]

    def __init__(
        self,
        ipot: int,
        z: int,
        tag: str,
        l_scmt: Optional[int] = None,
        l_fms: Optional[int] = None,
        stoich: Optional[float] = None,
    ) -> None: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...
    def __eq__(self, other: object) -> bool: ...

class Atom:
    x: float
    y: float
    z: float
    ipot: int
    tag: str
    distance: float

    def __init__(
        self,
        x: float,
        y: float,
        z: float,
        ipot: int,
        tag: str,
        distance: float = 0.0,
    ) -> None: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...
    def __eq__(self, other: object) -> bool: ...

class FeffInput:
    title: list[str]
    edge: Optional[str]
    s02: Optional[float]
    control: list[int]
    """6-element list of CONTROL flags."""
    print_flags: list[int]
    """6-element list of PRINT flags."""
    potentials: list[Potential]
    """Returns a cloned list. Cache in a local variable for repeated access."""
    atoms: list[Atom]
    """Returns a cloned list. Cache in a local variable for repeated access."""
    other_cards: list[str]
    num_potentials: int
    num_atoms: int

    def __init__(
        self,
        title: list[str] = ...,
        edge: Optional[str] = None,
        s02: Optional[float] = None,
        control: Optional[list[int]] = None,
        print_flags: Optional[list[int]] = None,
        potentials: list[Potential] = ...,
        atoms: list[Atom] = ...,
        other_cards: list[str] = ...,
    ) -> None: ...
    @staticmethod
    def parse(content: str) -> "FeffInput": ...
    @staticmethod
    def parse_strict(content: str) -> "FeffInput": ...
    @staticmethod
    def from_file(path: str) -> "FeffInput": ...
    @staticmethod
    def from_file_strict(path: str) -> "FeffInput": ...
    def to_string(self) -> str: ...
    def write_to_file(self, path: str) -> None: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class Stage:
    RDINP: "Stage"
    DMDW: "Stage"
    ATOMIC: "Stage"
    POT: "Stage"
    LDOS: "Stage"
    SCREEN: "Stage"
    CRPA: "Stage"
    OPCONSAT: "Stage"
    XSPH: "Stage"
    FMS: "Stage"
    MKGTR: "Stage"
    PATH: "Stage"
    GENFMT: "Stage"
    FF2X: "Stage"
    SFCONV: "Stage"
    COMPTON: "Stage"
    EELS: "Stage"
    RHORRP: "Stage"

    executable_name: str
    control_index: int

    @staticmethod
    def all() -> list["Stage"]: ...
    @staticmethod
    def default_pipeline() -> list["Stage"]: ...
    @staticmethod
    def from_name(name: str) -> "Stage": ...
    def __repr__(self) -> str: ...

class FeffConfig:
    work_dir: str
    stages: list[Stage]
    stage_timeout: Optional[float]

    def __init__(
        self,
        work_dir: str,
        input: FeffInput,
        stages: Optional[list[Stage]] = None,
        stage_timeout: Optional[float] = None,
    ) -> None: ...
    @staticmethod
    def from_file(
        work_dir: str,
        input_file: str,
        stages: Optional[list[Stage]] = None,
        stage_timeout: Optional[float] = None,
    ) -> "FeffConfig": ...
    def __repr__(self) -> str: ...

class StageProgress:
    kind: str
    """'starting' or 'finished'."""
    duration_secs: Optional[float]
    """Duration in seconds (only set when kind='finished')."""
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class StageResult:
    stage: Stage
    duration_secs: float
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class PipelineResult:
    stages: list[StageResult]
    work_dir: str
    total_duration_secs: float
    def outputs(self) -> "FeffOutputs": ...
    def read_xmu(self, strict: bool = False) -> "FeffTable": ...
    def read_chi(self, strict: bool = False) -> "FeffTable": ...
    def read_eels(self, strict: bool = False) -> "FeffTable": ...
    def read_ldos(self, index: int, strict: bool = False) -> "FeffTable": ...
    def read_paths(self) -> "PathsDat": ...
    def __repr__(self) -> str: ...

class FeffPipeline:
    def __init__(self, config: FeffConfig) -> None: ...
    def run(self) -> PipelineResult:
        """Run the full pipeline. Releases the GIL during computation."""
        ...
    def run_with_progress(
        self, callback: Callable[[Stage, StageProgress], None]
    ) -> PipelineResult:
        """Run with a progress callback. Releases the GIL between callbacks.

        If the callback raises an exception, it is re-raised after the
        current stage completes.
        """
        ...

class FeffTable:
    header: list[str]
    columns: list[list[float]]
    """Returns a cloned 2D list. Cache in a local variable for repeated access."""
    ncols: int
    nrows: int

    @staticmethod
    def parse(content: str) -> "FeffTable": ...
    @staticmethod
    def parse_strict(content: str) -> "FeffTable": ...
    @staticmethod
    def from_file(path: str) -> "FeffTable": ...
    @staticmethod
    def from_file_strict(path: str) -> "FeffTable": ...
    def column(self, index: int) -> list[float]: ...
    def r_squared(self, other: "FeffTable", col_x: int, col_y: int) -> float:
        """Compare spectra using R-squared metric. Returns NaN if no overlap."""
        ...
    def to_dataframe(self) -> object:
        """Convert to pandas DataFrame. Raises ImportError if pandas not installed."""
        ...
    def __getitem__(self, index: int) -> list[float]: ...
    def __iter__(self) -> Iterator[list[float]]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...
    def __len__(self) -> int: ...

class PathLeg:
    x: float
    y: float
    z: float
    ipot: int
    label: str
    rleg: float
    beta: float
    eta: float
    def __repr__(self) -> str: ...

class PathEntry:
    index: int
    nleg: int
    degeneracy: float
    r: float
    legs: list[PathLeg]
    def __repr__(self) -> str: ...

class PathsDat:
    header: list[str]
    entries: list[PathEntry]
    npaths: int
    @staticmethod
    def parse(content: str) -> "PathsDat": ...
    @staticmethod
    def from_file(path: str) -> "PathsDat": ...
    def total_degeneracy(self) -> float: ...
    def max_r(self) -> Optional[float]: ...
    def __len__(self) -> int: ...
    def __repr__(self) -> str: ...

class OutputFileInfo:
    name: str
    path: str
    kind: str
    def __repr__(self) -> str: ...

class FeffOutputs:
    work_dir: str
    files: list[OutputFileInfo]
    @staticmethod
    def discover(work_dir: str) -> "FeffOutputs": ...
    def read_table(self, name: str, strict: bool = False) -> FeffTable: ...
    def read_paths(self) -> PathsDat: ...
    def read_xmu(self, strict: bool = False) -> FeffTable: ...
    def read_chi(self, strict: bool = False) -> FeffTable: ...
    def read_eels(self, strict: bool = False) -> FeffTable: ...
    def read_ldos(self, index: int, strict: bool = False) -> FeffTable: ...
    def __len__(self) -> int: ...
    def __repr__(self) -> str: ...
