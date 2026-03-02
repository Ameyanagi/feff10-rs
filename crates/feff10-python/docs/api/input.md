# Input

## FeffInput

Represents a complete FEFF input file (`feff.inp`).

### Constructors

```python
FeffInput(
    title: list[str] = [],
    edge: Optional[str] = None,
    s02: Optional[float] = None,
    control: Optional[list[int]] = None,
    print_flags: Optional[list[int]] = None,
    potentials: list[Potential] = [],
    atoms: list[Atom] = [],
    other_cards: list[str] = [],
)
```

### Static Methods

| Method | Returns | Description |
|---|---|---|
| `parse(content: str)` | `FeffInput` | Parse from a string (permissive) |
| `parse_strict(content: str)` | `FeffInput` | Parse from a string (strict validation) |
| `from_file(path: str)` | `FeffInput` | Parse from a file path |
| `from_file_strict(path: str)` | `FeffInput` | Parse from a file path (strict) |

### Methods

| Method | Returns | Description |
|---|---|---|
| `to_string()` | `str` | Serialize to feff.inp format |
| `write_to_file(path: str)` | `None` | Write to a file path |

### Properties

| Property | Type | Writable | Description |
|---|---|---|---|
| `title` | `list[str]` | Yes | Title lines |
| `edge` | `Optional[str]` | Yes | Absorption edge (`"K"`, `"L3"`, etc.) |
| `s02` | `Optional[float]` | Yes | Amplitude reduction factor |
| `control` | `list[int]` | Yes | 6-element CONTROL flags |
| `print_flags` | `list[int]` | Yes | 6-element PRINT flags |
| `potentials` | `list[Potential]` | Yes | Scattering potentials (cloned on access) |
| `atoms` | `list[Atom]` | Yes | Atomic positions (cloned on access) |
| `other_cards` | `list[str]` | Yes | Other input cards (EXAFS, RPATH, etc.) |
| `num_potentials` | `int` | No | Number of potentials |
| `num_atoms` | `int` | No | Number of atoms |

---

## Potential

Defines a scattering potential in the POTENTIALS card.

### Constructor

```python
Potential(
    ipot: int,
    z: int,
    tag: str,
    l_scmt: Optional[int] = None,
    l_fms: Optional[int] = None,
    stoich: Optional[float] = None,
)
```

### Properties

| Property | Type | Writable | Description |
|---|---|---|---|
| `ipot` | `int` | Yes | Potential index (0 = absorber) |
| `z` | `int` | Yes | Atomic number |
| `tag` | `str` | Yes | Element tag |
| `l_scmt` | `Optional[int]` | Yes | Angular momentum for SCF |
| `l_fms` | `Optional[int]` | Yes | Angular momentum for FMS |
| `stoich` | `Optional[float]` | Yes | Stoichiometry factor |

Supports `==`, `repr()`, and `str()`.

---

## Atom

Defines an atomic position in the ATOMS card.

### Constructor

```python
Atom(
    x: float,
    y: float,
    z: float,
    ipot: int,
    tag: str,
    distance: float = 0.0,
)
```

### Properties

| Property | Type | Writable | Description |
|---|---|---|---|
| `x` | `float` | Yes | X coordinate (Angstroms) |
| `y` | `float` | Yes | Y coordinate (Angstroms) |
| `z` | `float` | Yes | Z coordinate (Angstroms) |
| `ipot` | `int` | Yes | Potential index |
| `tag` | `str` | Yes | Element tag |
| `distance` | `float` | Yes | Distance from absorber (Angstroms) |

Supports `==`, `repr()`, and `str()`.
