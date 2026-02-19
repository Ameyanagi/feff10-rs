pub const BOHR_ANGSTROM: f64 = 0.529_177_210_903;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DensityCommand {
    Line,
    Plane,
    Volume,
}

impl DensityCommand {
    pub fn ndims(self) -> usize {
        match self {
            Self::Line => 1,
            Self::Plane => 2,
            Self::Volume => 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DensityGrid {
    pub command: DensityCommand,
    pub filename: String,
    pub origin: [f64; 3],
    pub npts: Vec<usize>,
    pub axes: Vec<[f64; 3]>,
    pub core: bool,
}

impl DensityGrid {
    pub fn ndims(&self) -> usize {
        self.command.ndims()
    }
}

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum DensityInpError {
    #[error("unknown density grid type '{0}'")]
    UnknownCommand(String),
    #[error("missing field: {0}")]
    MissingField(&'static str),
    #[error("invalid float for {field}: {value}")]
    InvalidFloat { field: &'static str, value: String },
    #[error("invalid integer for {field}: {value}")]
    InvalidInteger { field: &'static str, value: String },
    #[error("axis rows for '{filename}' are incomplete")]
    IncompleteAxes { filename: String },
}

pub fn parse_density_inp(source: &str) -> Result<Vec<DensityGrid>, DensityInpError> {
    let lines = source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !is_comment_line(line))
        .collect::<Vec<_>>();

    let mut grids = Vec::new();
    let mut line_index = 0_usize;
    while line_index < lines.len() {
        let columns = lines[line_index].split_whitespace().collect::<Vec<_>>();
        if columns.is_empty() {
            line_index += 1;
            continue;
        }

        let command = parse_command(columns[0])?;
        if columns.len() < 5 {
            return Err(DensityInpError::MissingField(
                "command row requires: kind filename ox oy oz [core]",
            ));
        }

        let filename = columns[1].to_string();
        let origin = [
            parse_f64(columns[2], "origin.x")? / BOHR_ANGSTROM,
            parse_f64(columns[3], "origin.y")? / BOHR_ANGSTROM,
            parse_f64(columns[4], "origin.z")? / BOHR_ANGSTROM,
        ];
        let core = columns
            .get(5)
            .is_some_and(|value| value.eq_ignore_ascii_case("core"));

        let ndims = command.ndims();
        let mut axes = Vec::with_capacity(ndims);
        let mut npts = Vec::with_capacity(ndims);
        for _ in 0..ndims {
            line_index += 1;
            if line_index >= lines.len() {
                return Err(DensityInpError::IncompleteAxes {
                    filename: filename.clone(),
                });
            }
            let axis_columns = lines[line_index].split_whitespace().collect::<Vec<_>>();
            if axis_columns.len() < 4 {
                return Err(DensityInpError::MissingField(
                    "axis row requires: ax ay az npts",
                ));
            }
            axes.push([
                parse_f64(axis_columns[0], "axis.x")? / BOHR_ANGSTROM,
                parse_f64(axis_columns[1], "axis.y")? / BOHR_ANGSTROM,
                parse_f64(axis_columns[2], "axis.z")? / BOHR_ANGSTROM,
            ]);
            npts.push(parse_usize(axis_columns[3], "npts")?);
        }

        grids.push(DensityGrid {
            command,
            filename,
            origin,
            npts,
            axes,
            core,
        });
        line_index += 1;
    }

    Ok(grids)
}

pub fn density_inp_cleanup(grids: &mut Vec<DensityGrid>) {
    grids.clear();
}

pub fn is_command(word: &str) -> bool {
    matches!(word, "line" | "plane" | "volume")
}

fn parse_command(word: &str) -> Result<DensityCommand, DensityInpError> {
    match word.to_ascii_lowercase().as_str() {
        "line" => Ok(DensityCommand::Line),
        "plane" => Ok(DensityCommand::Plane),
        "volume" => Ok(DensityCommand::Volume),
        _ => Err(DensityInpError::UnknownCommand(word.to_string())),
    }
}

fn parse_f64(value: &str, field: &'static str) -> Result<f64, DensityInpError> {
    value
        .replace(['D', 'd'], "E")
        .parse::<f64>()
        .map_err(|_| DensityInpError::InvalidFloat {
            field,
            value: value.to_string(),
        })
}

fn parse_usize(value: &str, field: &'static str) -> Result<usize, DensityInpError> {
    value
        .parse::<usize>()
        .map_err(|_| DensityInpError::InvalidInteger {
            field,
            value: value.to_string(),
        })
}

fn is_comment_line(line: &str) -> bool {
    line.chars()
        .next()
        .is_some_and(|character| matches!(character, '#' | '!' | '*' | 'C' | 'c'))
}

#[cfg(test)]
mod tests {
    use super::{
        BOHR_ANGSTROM, DensityCommand, DensityInpError, density_inp_cleanup, is_command,
        parse_density_inp,
    };

    #[test]
    fn parse_density_inp_reads_grid_records() {
        let source = "\
line density_line.dat 0.0 0.0 0.0 core
1.0 0.0 0.0 5
plane density_plane.dat 0.0 0.0 0.0
1.0 0.0 0.0 2
0.0 1.0 0.0 3
";
        let grids = parse_density_inp(source).expect("density.inp should parse");
        assert_eq!(grids.len(), 2);
        assert_eq!(grids[0].command, DensityCommand::Line);
        assert_eq!(grids[0].npts, vec![5]);
        assert!(grids[0].core);
        assert!((grids[0].axes[0][0] - 1.0 / BOHR_ANGSTROM).abs() <= 1.0e-12);
    }

    #[test]
    fn parse_density_inp_rejects_unknown_command() {
        let source = "sphere density.dat 0.0 0.0 0.0\n";
        let error = parse_density_inp(source).expect_err("unknown command should fail");
        assert_eq!(error, DensityInpError::UnknownCommand("sphere".to_string()));
    }

    #[test]
    fn cleanup_clears_allocated_grids() {
        let source = "line density_line.dat 0.0 0.0 0.0\n1.0 0.0 0.0 5\n";
        let mut grids = parse_density_inp(source).expect("density.inp should parse");
        density_inp_cleanup(&mut grids);
        assert!(grids.is_empty());
    }

    #[test]
    fn is_command_matches_supported_grid_types() {
        assert!(is_command("line"));
        assert!(is_command("plane"));
        assert!(is_command("volume"));
        assert!(!is_command("cube"));
    }
}
