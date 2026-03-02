# Parsing Output

The `XmuDat` class parses FEFF output files (`xmu.dat`) and provides column-oriented access to spectral data.

## Reading Output Files

```python
import feff10

# From a file path
xmu = feff10.XmuDat.from_file("./work/xmu.dat")

# From a string
content = open("xmu.dat").read()
xmu = feff10.XmuDat.parse(content)

# Strict mode — rejects ragged rows and invalid numbers
xmu = feff10.XmuDat.from_file_strict("xmu.dat")
```

## Inspecting Data

```python
xmu = feff10.XmuDat.from_file("xmu.dat")

print(xmu.ncols)    # number of columns
print(xmu.nrows)    # number of data points
print(len(xmu))     # same as nrows
print(xmu.header)   # list of comment lines from the file header

# Preview the data
print(xmu)  # shows first 5 rows
```

## Accessing Columns

```python
# By method
energy = xmu.column(0)
mu = xmu.column(3)

# By index (supports negative indexing)
first_col = xmu[0]
last_col = xmu[-1]

# All columns at once (cloned — cache for repeated use)
cols = xmu.columns
```

## Iteration

```python
# Iterate over columns
for col in xmu:
    print(f"Column with {len(col)} points, range [{col[0]:.2f}, {col[-1]:.2f}]")
```

## Comparing Spectra

The R-squared metric quantifies the difference between two spectra by interpolating both onto a common energy grid:

```python
calculated = feff10.XmuDat.from_file("./work/xmu.dat")
reference = feff10.XmuDat.from_file("reference_xmu.dat")

# col_x=0 is energy, col_y=3 is the spectrum to compare
rsq = calculated.r_squared(reference, col_x=0, col_y=3)
print(f"R-squared = {rsq*100:.4f}%")  # lower is better
```

!!! note
    `r_squared()` returns `NaN` if the two spectra have no overlapping energy range.

## Pandas Integration

Convert to a pandas DataFrame for further analysis:

```python
xmu = feff10.XmuDat.from_file("xmu.dat")
df = xmu.to_dataframe()  # requires pandas
print(df.describe())
```

!!! warning
    `to_dataframe()` raises `ImportError` if pandas is not installed.
    Install with: `uv pip install "feff10[pandas]"`

## Full Workflow Example

```python
import feff10

# Run a calculation
inp = feff10.FeffInput.from_file("feff.inp")
config = feff10.FeffConfig("./work", inp)
result = feff10.FeffPipeline(config).run()

# Parse and analyze the output
xmu = feff10.XmuDat.from_file("./work/xmu.dat")
energy = xmu[0]
mu = xmu[3]

# Compare with a reference
ref = feff10.XmuDat.from_file("reference.dat")
rsq = xmu.r_squared(ref, col_x=0, col_y=3)
print(f"Deviation from reference: {rsq*100:.4f}%")
```
