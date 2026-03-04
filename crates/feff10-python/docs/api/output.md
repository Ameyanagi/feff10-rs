# Output

## FeffTable

Parsed FEFF output file (`xmu.dat`). Provides column-oriented access to spectral data.

### Static Methods

| Method | Returns | Description |
|---|---|---|
| `parse(content: str)` | `FeffTable` | Parse from a string (permissive) |
| `parse_strict(content: str)` | `FeffTable` | Parse from a string (strict — rejects ragged rows, invalid numbers) |
| `from_file(path: str)` | `FeffTable` | Parse from a file path |
| `from_file_strict(path: str)` | `FeffTable` | Parse from a file path (strict) |

### Properties

| Property | Type | Description |
|---|---|---|
| `header` | `list[str]` | Comment lines from the file header |
| `columns` | `list[list[float]]` | All columns as a 2D list (cloned on access) |
| `ncols` | `int` | Number of columns |
| `nrows` | `int` | Number of data points (rows) |

### Methods

#### `column(index: int) -> list[float]`

Get a specific column by 0-based index. Raises `IndexError` if out of range.

#### `r_squared(other: FeffTable, col_x: int, col_y: int) -> float`

Compare two spectra using the R-squared metric. Both spectra are interpolated onto a common energy grid (100 points in the overlapping range).

- `col_x` — column index for the x-axis (energy)
- `col_y` — column index for the y-axis (spectrum)
- Returns `NaN` if the spectra have no overlapping energy range

#### `to_dataframe() -> DataFrame`

Convert to a pandas DataFrame. Columns are named `col_0`, `col_1`, etc.

Raises `ImportError` if pandas is not installed.

### Protocols

| Protocol | Description |
|---|---|
| `xmu[i]` | Get column by index (supports negative indexing) |
| `for col in xmu` | Iterate over columns |
| `len(xmu)` | Number of rows |
| `str(xmu)` | Preview first 5 rows |
| `repr(xmu)` | Summary string |
