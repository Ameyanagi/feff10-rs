use std::collections::{BTreeMap, HashSet};
use std::io::{BufRead, Write as _};
use std::path::PathBuf;
use std::time::Duration;

use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use serde::Serialize;

use feff10::config::FeffConfigBuilder;
use feff10::input::FeffInput;
use feff10::output::XmuDat;
use feff10::pipeline::{FeffPipeline, StageProgress};
use feff10::stage::Stage;

// --- Embedded examples ---

const EXAMPLES: &[(&str, &str, &str)] = &[
    (
        "exafs-cu",
        "EXAFS Cu crystal (standard reference, 79 atoms)",
        include_str!("../examples/bundled/exafs-cu.inp"),
    ),
    (
        "exafs-sf6",
        "EXAFS SF6 molecule (minimal, 7 atoms)",
        include_str!("../examples/bundled/exafs-sf6.inp"),
    ),
    (
        "xanes-cu",
        "XANES Cu crystal (FMS calculation)",
        include_str!("../examples/bundled/xanes-cu.inp"),
    ),
    (
        "xanes-bn",
        "XANES BN compound (705 atoms)",
        include_str!("../examples/bundled/xanes-bn.inp"),
    ),
    (
        "xes-cu",
        "XES Cu crystal",
        include_str!("../examples/bundled/xes-cu.inp"),
    ),
    (
        "exafs-ybco",
        "EXAFS YBCO superconductor (multi-element)",
        include_str!("../examples/bundled/exafs-ybco.inp"),
    ),
    (
        "exafs-gecl4",
        "EXAFS GeCl4 molecule",
        include_str!("../examples/bundled/exafs-gecl4.inp"),
    ),
    (
        "fprime-gecl4",
        "FPRIME GeCl4 (anomalous scattering)",
        include_str!("../examples/bundled/fprime-gecl4.inp"),
    ),
];

// --- Element table (symbol -> Z) ---

const ELEMENTS: &[(&str, u32)] = &[
    ("H", 1),
    ("He", 2),
    ("Li", 3),
    ("Be", 4),
    ("B", 5),
    ("C", 6),
    ("N", 7),
    ("O", 8),
    ("F", 9),
    ("Ne", 10),
    ("Na", 11),
    ("Mg", 12),
    ("Al", 13),
    ("Si", 14),
    ("P", 15),
    ("S", 16),
    ("Cl", 17),
    ("Ar", 18),
    ("K", 19),
    ("Ca", 20),
    ("Sc", 21),
    ("Ti", 22),
    ("V", 23),
    ("Cr", 24),
    ("Mn", 25),
    ("Fe", 26),
    ("Co", 27),
    ("Ni", 28),
    ("Cu", 29),
    ("Zn", 30),
    ("Ga", 31),
    ("Ge", 32),
    ("As", 33),
    ("Se", 34),
    ("Br", 35),
    ("Kr", 36),
    ("Rb", 37),
    ("Sr", 38),
    ("Y", 39),
    ("Zr", 40),
    ("Nb", 41),
    ("Mo", 42),
    ("Tc", 43),
    ("Ru", 44),
    ("Rh", 45),
    ("Pd", 46),
    ("Ag", 47),
    ("Cd", 48),
    ("In", 49),
    ("Sn", 50),
    ("Sb", 51),
    ("Te", 52),
    ("I", 53),
    ("Xe", 54),
    ("Cs", 55),
    ("Ba", 56),
    ("La", 57),
    ("Ce", 58),
    ("Pr", 59),
    ("Nd", 60),
    ("Pm", 61),
    ("Sm", 62),
    ("Eu", 63),
    ("Gd", 64),
    ("Tb", 65),
    ("Dy", 66),
    ("Ho", 67),
    ("Er", 68),
    ("Tm", 69),
    ("Yb", 70),
    ("Lu", 71),
    ("Hf", 72),
    ("Ta", 73),
    ("W", 74),
    ("Re", 75),
    ("Os", 76),
    ("Ir", 77),
    ("Pt", 78),
    ("Au", 79),
    ("Hg", 80),
    ("Tl", 81),
    ("Pb", 82),
    ("Bi", 83),
    ("Po", 84),
    ("At", 85),
    ("Rn", 86),
    ("Fr", 87),
    ("Ra", 88),
    ("Ac", 89),
    ("Th", 90),
    ("Pa", 91),
    ("U", 92),
    ("Np", 93),
    ("Pu", 94),
    ("Am", 95),
    ("Cm", 96),
    ("Bk", 97),
    ("Cf", 98),
    ("Es", 99),
    ("Fm", 100),
];

fn element_symbol_to_z(input: &str) -> Option<(String, u32)> {
    // Try parsing as Z number first
    if let Ok(z) = input.parse::<u32>() {
        return ELEMENTS
            .iter()
            .find(|(_, ez)| *ez == z)
            .map(|(sym, z)| (sym.to_string(), *z));
    }
    // Try matching as symbol (case-insensitive)
    ELEMENTS
        .iter()
        .find(|(sym, _)| sym.eq_ignore_ascii_case(input))
        .map(|(sym, z)| (sym.to_string(), *z))
}

fn long_version() -> &'static str {
    concat!(
        env!("CARGO_PKG_VERSION"),
        "\n",
        "FEFF10 commit:    ",
        env!("FEFF10_COMMIT"),
        "\n",
        "Fortran compiler: ",
        env!("FEFF10_FC"),
        "\n",
        "Fortran flags:    ",
        env!("FEFF10_FFLAGS"),
        "\n",
        "BLAS/LAPACK:      ",
        env!("FEFF10_BLAS"),
    )
}

#[derive(Parser)]
#[command(
    name = "feff10-rs",
    about = "FEFF10 X-ray absorption spectroscopy calculations",
    version = long_version(),
)]
struct Cli {
    #[command(flatten)]
    global: GlobalArgs,

    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Args)]
struct GlobalArgs {
    /// Suppress progress bars and human-friendly output
    #[arg(short, long, global = true)]
    quiet: bool,

    /// Show verbose output (error details, exit codes)
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Machine-readable JSON output
    #[arg(long, global = true)]
    json: bool,
}

#[derive(clap::Subcommand)]
enum Command {
    /// Run a FEFF calculation
    Run {
        /// Path to feff.inp file or directory containing it
        #[arg(default_value = ".")]
        input: PathBuf,
        /// Working directory (defaults to current directory or input's directory)
        #[arg(short, long)]
        work_dir: Option<PathBuf>,
        /// Only run specific stages (comma-separated)
        #[arg(short, long, value_delimiter = ',')]
        stages: Option<Vec<String>>,
        /// Kill a stage if it exceeds this many seconds
        #[arg(long)]
        timeout: Option<u64>,
    },
    /// Parse and validate a feff.inp file
    Validate {
        /// Path to feff.inp file
        #[arg(default_value = "feff.inp")]
        input: PathBuf,
    },
    /// Compare two xmu.dat (or similar) files using R-squared metric
    Compare {
        /// First spectrum file
        file1: PathBuf,
        /// Second spectrum file
        file2: PathBuf,
        /// X-column index (1-based, default: 1)
        #[arg(long, default_value = "1")]
        col_x: usize,
        /// Y-column index (1-based, default: 4)
        #[arg(long, default_value = "4")]
        col_y: usize,
        /// Pass/fail threshold percentage (default: 0.1)
        #[arg(long, default_value = "0.1")]
        threshold: f64,
    },
    /// Benchmark FEFF on one or more examples (multiple iterations, JSON output)
    Bench {
        /// Paths to feff.inp files or directories containing them
        #[arg(required = true)]
        inputs: Vec<PathBuf>,
        /// Number of iterations per example
        #[arg(short = 'n', long, default_value = "3")]
        iterations: usize,
        /// Output JSON results to this file (in addition to stdout summary)
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Label for this benchmark run (e.g. "gfortran-O3")
        #[arg(short, long, default_value = "default")]
        label: String,
    },
    /// List all FEFF pipeline stages
    Stages,
    /// List or copy bundled example feff.inp files
    Examples {
        /// Example name to copy (omit to list all)
        name: Option<String>,
        /// Output directory (default: current directory)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Generate a feff.inp template (interactive without args)
    Init {
        /// Calculation type (exafs, xanes, xes)
        #[arg(short = 't', long = "type")]
        calc_type: Option<String>,
        /// Absorption edge (K, L1, L2, L3)
        #[arg(short, long)]
        edge: Option<String>,
        /// Absorber element (symbol or Z number)
        #[arg(long)]
        element: Option<String>,
        /// Output file path (default: ./feff.inp)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

// --- JSON output types ---

#[derive(Serialize)]
struct RunOutput {
    work_dir: String,
    total_secs: f64,
    stages: Vec<StageTimingOutput>,
}

#[derive(Serialize)]
struct StageTimingOutput {
    name: String,
    duration_secs: f64,
}

#[derive(Serialize)]
struct ValidateOutput {
    path: String,
    valid: bool,
    edge: Option<String>,
    s02: Option<f64>,
    control: [u32; 6],
    print_flags: [u32; 6],
    potentials: Vec<PotentialOutput>,
    atoms_count: usize,
    active_stages: Vec<String>,
}

#[derive(Serialize)]
struct PotentialOutput {
    ipot: u32,
    z: u32,
    tag: String,
    l_scmt: Option<u32>,
    l_fms: Option<u32>,
    stoich: Option<f64>,
}

#[derive(Serialize)]
struct CompareOutput {
    file1: String,
    file2: String,
    col_x: usize,
    col_y: usize,
    r_squared_pct: f64,
    threshold_pct: f64,
    pass: bool,
}

#[derive(Serialize)]
struct StageInfoOutput {
    name: String,
    control_group: usize,
    order: usize,
}

#[derive(Serialize)]
struct ExampleInfoOutput {
    name: String,
    description: String,
}

// --- Benchmark types ---

#[derive(Debug, Serialize)]
struct BenchReport {
    label: String,
    examples: Vec<ExampleBench>,
}

#[derive(Debug, Serialize)]
struct ExampleBench {
    name: String,
    iterations: usize,
    /// Per-stage timings: stage_name -> [durations in seconds per iteration]
    stages: BTreeMap<String, Vec<f64>>,
    /// Total pipeline time per iteration (seconds)
    total_secs: Vec<f64>,
    /// Stats
    mean_total_secs: f64,
    min_total_secs: f64,
    max_total_secs: f64,
    /// Per-stage mean
    stage_means: BTreeMap<String, f64>,
}

type CliResult = Result<(), i32>;

fn emit_json<T: Serialize>(writer: &mut dyn std::io::Write, value: &T) -> CliResult {
    match serde_json::to_string_pretty(value) {
        Ok(json) => {
            let _ = writeln!(writer, "{json}");
            Ok(())
        }
        Err(e) => {
            eprintln!("Error serializing JSON output: {e}");
            Err(1)
        }
    }
}

fn print_json<T: Serialize>(value: &T) -> CliResult {
    emit_json(&mut std::io::stdout().lock(), value)
}

fn eprint_json<T: Serialize>(value: &T) -> CliResult {
    emit_json(&mut std::io::stderr().lock(), value)
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Command::Run {
            input,
            work_dir,
            stages,
            timeout,
        } => cmd_run(input, work_dir, stages, timeout, &cli.global),
        Command::Validate { input } => cmd_validate(input, &cli.global),
        Command::Compare {
            file1,
            file2,
            col_x,
            col_y,
            threshold,
        } => cmd_compare(file1, file2, col_x, col_y, threshold, &cli.global),
        Command::Bench {
            inputs,
            iterations,
            output,
            label,
        } => cmd_bench(inputs, iterations, output, label, &cli.global),
        Command::Stages => cmd_stages(&cli.global),
        Command::Examples { name, output } => cmd_examples(name, output, &cli.global),
        Command::Init {
            calc_type,
            edge,
            element,
            output,
        } => cmd_init(calc_type, edge, element, output, &cli.global),
    };

    if let Err(code) = result {
        std::process::exit(code);
    }
}

fn parse_requested_stages(stage_names: &[String]) -> Result<Vec<Stage>, String> {
    let mut stages = Vec::new();
    let mut seen = HashSet::new();
    let mut unknown = Vec::new();

    for name in stage_names {
        match name.parse::<Stage>() {
            Ok(stage) => {
                if seen.insert(stage) {
                    stages.push(stage);
                }
            }
            Err(_) => unknown.push(name.clone()),
        }
    }

    if !unknown.is_empty() {
        let allowed = Stage::all()
            .iter()
            .map(|s| s.executable_name())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "Unknown stage(s): {}. Allowed values: {allowed}",
            unknown.join(", ")
        ));
    }

    Ok(stages)
}

fn normalize_compare_columns(col_x: usize, col_y: usize) -> Result<(usize, usize), String> {
    if col_x == 0 || col_y == 0 {
        return Err("--col-x and --col-y must be >= 1".to_string());
    }
    Ok((col_x - 1, col_y - 1))
}

fn cmd_run(
    input: PathBuf,
    work_dir: Option<PathBuf>,
    stage_names: Option<Vec<String>>,
    timeout: Option<u64>,
    global: &GlobalArgs,
) -> CliResult {
    let inp_path = if input.is_dir() {
        input.join("feff.inp")
    } else {
        input.clone()
    };

    if !inp_path.exists() {
        eprintln!("Error: {} not found", inp_path.display());
        return Err(1);
    }

    let work = work_dir.unwrap_or_else(|| {
        inp_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .to_path_buf()
    });

    let feff_input = match FeffInput::from_file(&inp_path) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("Error parsing {}: {e}", inp_path.display());
            return Err(1);
        }
    };

    let mut builder = FeffConfigBuilder::new().work_dir(&work).input(feff_input);

    if let Some(names) = stage_names {
        let stages = match parse_requested_stages(&names) {
            Ok(stages) => stages,
            Err(msg) => {
                eprintln!("Error: {msg}");
                return Err(2);
            }
        };
        builder = builder.stages(stages);
    }

    if let Some(secs) = timeout {
        builder = builder.stage_timeout(Duration::from_secs(secs));
    }

    let config = match builder.build() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Config error: {e}");
            return Err(1);
        }
    };

    let total = config.stages.len() as u64;
    let pb = if global.quiet || global.json {
        ProgressBar::hidden()
    } else {
        let pb = ProgressBar::new(total);
        if let Ok(style) =
            ProgressStyle::default_bar().template("[{elapsed_precise}] {bar:40} {pos}/{len} {msg}")
        {
            pb.set_style(style);
        }
        pb
    };

    let pipeline = FeffPipeline::new(config);
    let result = pipeline.run_with_progress(|stage, progress| match progress {
        StageProgress::Starting => {
            pb.set_message(format!("Running {stage}..."));
        }
        StageProgress::Finished { duration } => {
            pb.set_message(format!("{stage} done ({:.1}s)", duration.as_secs_f64()));
            pb.inc(1);
        }
    });

    pb.finish_and_clear();

    match result {
        Ok(res) => {
            if global.json {
                let output = RunOutput {
                    work_dir: res.work_dir.display().to_string(),
                    total_secs: res.stages.iter().map(|s| s.duration.as_secs_f64()).sum(),
                    stages: res
                        .stages
                        .iter()
                        .map(|s| StageTimingOutput {
                            name: s.stage.executable_name().to_string(),
                            duration_secs: s.duration.as_secs_f64(),
                        })
                        .collect(),
                };
                print_json(&output)?;
            } else if !global.quiet {
                println!(
                    "FEFF calculation completed successfully in {}",
                    res.work_dir.display()
                );
                let total_time: Duration = res.stages.iter().map(|s| s.duration).sum();
                println!("Total time: {:.1}s", total_time.as_secs_f64());
                for sr in &res.stages {
                    println!(
                        "  {:>10} {:.1}s",
                        sr.stage.executable_name(),
                        sr.duration.as_secs_f64()
                    );
                }
            }
        }
        Err(e) => {
            if global.json {
                let err_json = match &e {
                    feff10::error::Error::Pipeline(pe) => serde_json::json!({
                        "error": e.to_string(),
                        "stage": pe.stage,
                        "exit_code": pe.exit_code,
                        "feff_error": pe.feff_error,
                    }),
                    _ => serde_json::json!({ "error": e.to_string() }),
                };
                eprint_json(&err_json)?;
            } else {
                eprintln!("Error: {e}");
                if global.verbose
                    && let feff10::error::Error::Pipeline(pe) = &e
                {
                    if let Some(ref fe) = pe.feff_error {
                        eprintln!("\n--- .feff.error ---\n{fe}");
                    }
                    if let Some(code) = pe.exit_code {
                        eprintln!("Exit code: {code}");
                    }
                }
            }
            return Err(1);
        }
    }

    Ok(())
}

fn cmd_validate(input: PathBuf, global: &GlobalArgs) -> CliResult {
    match FeffInput::from_file_strict(&input) {
        Ok(inp) => {
            let active: Vec<String> = Stage::default_pipeline()
                .iter()
                .filter(|s| inp.control[s.control_index()] != 0)
                .map(|s| s.executable_name().to_string())
                .collect();

            if global.json {
                let output = ValidateOutput {
                    path: input.display().to_string(),
                    valid: true,
                    edge: inp.edge.clone(),
                    s02: inp.s02,
                    control: inp.control,
                    print_flags: inp.print_flags,
                    potentials: inp
                        .potentials
                        .iter()
                        .map(|p| PotentialOutput {
                            ipot: p.ipot,
                            z: p.z,
                            tag: p.tag.clone(),
                            l_scmt: p.l_scmt,
                            l_fms: p.l_fms,
                            stoich: p.stoich,
                        })
                        .collect(),
                    atoms_count: inp.atoms.len(),
                    active_stages: active,
                };
                print_json(&output)?;
            } else if !global.quiet {
                println!("Valid feff.inp: {}", input.display());
                if let Some(ref edge) = inp.edge {
                    println!("  Edge: {edge}");
                }
                println!("  Potentials: {}", inp.potentials.len());
                println!("  Atoms: {}", inp.atoms.len());
                println!(
                    "  CONTROL: {} {} {} {} {} {}",
                    inp.control[0],
                    inp.control[1],
                    inp.control[2],
                    inp.control[3],
                    inp.control[4],
                    inp.control[5]
                );
                println!("  Active stages: {}", active.join(", "));
            }
        }
        Err(e) => {
            if global.json {
                let err_json = serde_json::json!({
                    "path": input.display().to_string(),
                    "valid": false,
                    "error": e.to_string(),
                });
                print_json(&err_json)?;
            } else {
                eprintln!("Error: {e}");
            }
            return Err(1);
        }
    }

    Ok(())
}

fn cmd_compare(
    file1: PathBuf,
    file2: PathBuf,
    col_x: usize,
    col_y: usize,
    threshold: f64,
    global: &GlobalArgs,
) -> CliResult {
    let (col_x_zero, col_y_zero) = match normalize_compare_columns(col_x, col_y) {
        Ok(cols) => cols,
        Err(msg) => {
            eprintln!("Error: {msg}");
            return Err(2);
        }
    };

    let xmu1 = match XmuDat::from_file_strict(&file1) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("Error reading {}: {e}", file1.display());
            return Err(1);
        }
    };
    let xmu2 = match XmuDat::from_file_strict(&file2) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("Error reading {}: {e}", file2.display());
            return Err(1);
        }
    };

    let rsq = xmu1.r_squared(&xmu2, col_x_zero, col_y_zero);
    let pct = rsq * 100.0;
    let pass = pct < threshold;

    if global.json {
        let output = CompareOutput {
            file1: file1.display().to_string(),
            file2: file2.display().to_string(),
            col_x,
            col_y,
            r_squared_pct: pct,
            threshold_pct: threshold,
            pass,
        };
        print_json(&output)?;
    } else if !global.quiet {
        println!("R-squared comparison (columns {col_x} vs {col_y}):");
        println!("  {} vs {}", file1.display(), file2.display());
        println!("  Average deviation: {pct:.6}%");
        if pass {
            println!("  Result: PASS (< {threshold}%)");
        } else {
            println!("  Result: FAIL (>= {threshold}%)");
        }
    }

    if !pass {
        return Err(1);
    }

    Ok(())
}

fn cmd_bench(
    inputs: Vec<PathBuf>,
    iterations: usize,
    output: Option<PathBuf>,
    label: String,
    global: &GlobalArgs,
) -> CliResult {
    let iterations = iterations.max(1);
    let mut report = BenchReport {
        label: label.clone(),
        examples: Vec::new(),
    };

    for input in &inputs {
        let inp_path = if input.is_dir() {
            input.join("feff.inp")
        } else {
            input.clone()
        };
        if !inp_path.exists() {
            eprintln!("Warning: {} not found, skipping", inp_path.display());
            continue;
        }

        let example_name = inp_path
            .parent()
            .and_then(|p| {
                let components: Vec<_> = p.components().rev().take(2).collect();
                if components.len() == 2 {
                    Some(format!(
                        "{}/{}",
                        components[1].as_os_str().to_string_lossy(),
                        components[0].as_os_str().to_string_lossy()
                    ))
                } else {
                    Some(p.file_name()?.to_string_lossy().to_string())
                }
            })
            .unwrap_or_else(|| "unknown".to_string());

        if !global.quiet {
            eprintln!("Benchmarking {example_name} ({iterations} iterations)...");
        }

        let mut stage_timings: BTreeMap<String, Vec<f64>> = BTreeMap::new();
        let mut total_times: Vec<f64> = Vec::new();

        for iter in 0..iterations {
            let run_iteration =
                |stage_timings: &mut BTreeMap<String, Vec<f64>>| -> Result<f64, String> {
                    let work_dir = tempfile::tempdir().map_err(|e| format!("tempdir: {e}"))?;
                    std::fs::copy(&inp_path, work_dir.path().join("feff.inp"))
                        .map_err(|e| format!("copy: {e}"))?;
                    let feff_input = FeffInput::from_file(work_dir.path().join("feff.inp"))
                        .map_err(|e| format!("input parse: {e}"))?;
                    let config = FeffConfigBuilder::new()
                        .work_dir(work_dir.path())
                        .input(feff_input)
                        .build()
                        .map_err(|e| format!("config: {e}"))?;
                    let res = FeffPipeline::new(config)
                        .run()
                        .map_err(|e| e.to_string())?;
                    let mut total = 0.0;
                    for sr in &res.stages {
                        let secs = sr.duration.as_secs_f64();
                        stage_timings
                            .entry(sr.stage.executable_name().to_string())
                            .or_default()
                            .push(secs);
                        total += secs;
                    }
                    Ok(total)
                };

            match run_iteration(&mut stage_timings) {
                Ok(total) => {
                    total_times.push(total);
                    if !global.quiet {
                        eprintln!("  iteration {}/{}: {:.2}s", iter + 1, iterations, total);
                    }
                }
                Err(msg) => {
                    if !global.quiet {
                        eprintln!("  iteration {}/{}: FAILED - {msg}", iter + 1, iterations);
                    }
                    total_times.push(f64::NAN);
                }
            }
        }

        let valid_totals: Vec<f64> = total_times
            .iter()
            .copied()
            .filter(|t| t.is_finite())
            .collect();
        let mean = if valid_totals.is_empty() {
            f64::NAN
        } else {
            valid_totals.iter().sum::<f64>() / valid_totals.len() as f64
        };
        let min = valid_totals.iter().copied().fold(f64::INFINITY, f64::min);
        let max = valid_totals
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);

        let stage_means: BTreeMap<String, f64> = stage_timings
            .iter()
            .map(|(k, v)| {
                let valid: Vec<f64> = v.iter().copied().filter(|t| t.is_finite()).collect();
                let m = if valid.is_empty() {
                    f64::NAN
                } else {
                    valid.iter().sum::<f64>() / valid.len() as f64
                };
                (k.clone(), m)
            })
            .collect();

        report.examples.push(ExampleBench {
            name: example_name,
            iterations,
            stages: stage_timings,
            total_secs: total_times,
            mean_total_secs: mean,
            min_total_secs: min,
            max_total_secs: max,
            stage_means,
        });
    }

    if global.json {
        print_json(&report)?;
    } else if !global.quiet {
        println!();
        println!("=== Benchmark Results: {label} ===");
        println!();
        println!(
            "{:<20} {:>8} {:>10} {:>10} {:>10}",
            "Example", "Iters", "Mean (s)", "Min (s)", "Max (s)"
        );
        println!("{}", "-".repeat(62));
        for ex in &report.examples {
            println!(
                "{:<20} {:>8} {:>10.3} {:>10.3} {:>10.3}",
                ex.name, ex.iterations, ex.mean_total_secs, ex.min_total_secs, ex.max_total_secs
            );
        }
        println!();

        for ex in &report.examples {
            println!("--- {} stage breakdown (mean) ---", ex.name);
            for (stage, mean) in &ex.stage_means {
                println!("  {stage:>12}: {mean:.3}s");
            }
            println!();
        }
    }

    if let Some(out_path) = output {
        let mut file = match std::fs::File::create(&out_path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Error creating {}: {e}", out_path.display());
                return Err(1);
            }
        };
        emit_json(&mut file, &report)?;
        if !global.quiet {
            eprintln!("JSON results written to {}", out_path.display());
        }
    }

    Ok(())
}

fn cmd_examples(name: Option<String>, output: Option<PathBuf>, global: &GlobalArgs) -> CliResult {
    match name {
        None => {
            // List all examples
            if global.json {
                let list: Vec<ExampleInfoOutput> = EXAMPLES
                    .iter()
                    .map(|(name, desc, _)| ExampleInfoOutput {
                        name: name.to_string(),
                        description: desc.to_string(),
                    })
                    .collect();
                print_json(&list)?;
            } else if !global.quiet {
                println!("Available examples:");
                println!();
                for (name, desc, _) in EXAMPLES {
                    println!("  {name:<18} {desc}");
                }
                println!();
                println!("Usage: feff10-rs examples <name> [-o <dir>]");
            }
        }
        Some(ref key) => {
            let example = EXAMPLES.iter().find(|(name, _, _)| name == key);
            match example {
                Some((name, desc, content)) => {
                    let out_dir = output.unwrap_or_else(|| PathBuf::from("."));
                    if let Err(e) = std::fs::create_dir_all(&out_dir) {
                        eprintln!("Error creating directory {}: {e}", out_dir.display());
                        return Err(1);
                    }
                    let out_path = out_dir.join("feff.inp");
                    if let Err(e) = std::fs::write(&out_path, content) {
                        eprintln!("Error writing {}: {e}", out_path.display());
                        return Err(1);
                    }
                    if global.json {
                        let info = serde_json::json!({
                            "name": name,
                            "description": desc,
                            "path": out_path.display().to_string(),
                        });
                        print_json(&info)?;
                    } else if !global.quiet {
                        println!("Copied {name} ({desc}) to {}", out_path.display());
                    }
                }
                None => {
                    eprintln!("Unknown example: {key}");
                    eprintln!(
                        "Available: {}",
                        EXAMPLES
                            .iter()
                            .map(|(n, _, _)| *n)
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                    return Err(2);
                }
            }
        }
    }

    Ok(())
}

fn cmd_init(
    calc_type: Option<String>,
    edge: Option<String>,
    element: Option<String>,
    output: Option<PathBuf>,
    global: &GlobalArgs,
) -> CliResult {
    let stdin = std::io::stdin();
    let mut reader = stdin.lock();

    let calc_type = calc_type.unwrap_or_else(|| {
        prompt_choice(&mut reader, "Calculation type", &["EXAFS", "XANES", "XES"])
    });
    let calc_type_upper = calc_type.to_uppercase();

    let edge = edge
        .unwrap_or_else(|| prompt_choice(&mut reader, "Absorption edge", &["K", "L1", "L2", "L3"]));
    let edge_upper = edge.to_uppercase();

    let element =
        element.unwrap_or_else(|| prompt_input(&mut reader, "Absorber element (symbol or Z)"));

    let (symbol, z) = match element_symbol_to_z(&element) {
        Some(pair) => pair,
        None => {
            eprintln!("Unknown element: {element}");
            return Err(2);
        }
    };

    let template = generate_template(&calc_type_upper, &edge_upper, &symbol, z);

    let out_path = output.unwrap_or_else(|| PathBuf::from("feff.inp"));
    if let Some(parent) = out_path.parent()
        && !parent.as_os_str().is_empty()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        eprintln!("Error creating directory {}: {e}", parent.display());
        return Err(1);
    }
    if let Err(e) = std::fs::write(&out_path, &template) {
        eprintln!("Error writing {}: {e}", out_path.display());
        return Err(1);
    }

    if global.json {
        let info = serde_json::json!({
            "path": out_path.display().to_string(),
            "type": calc_type_upper,
            "edge": edge_upper,
            "element": symbol,
            "z": z,
        });
        print_json(&info)?;
    } else if !global.quiet {
        println!(
            "Generated {} {} {}-edge template: {}",
            calc_type_upper,
            symbol,
            edge_upper,
            out_path.display()
        );
    }

    Ok(())
}

fn prompt_choice(reader: &mut impl BufRead, label: &str, options: &[&str]) -> String {
    eprintln!("{label}:");
    for (i, opt) in options.iter().enumerate() {
        eprintln!("  {}: {opt}", i + 1);
    }
    eprint!("Select [1-{}]: ", options.len());
    std::io::stderr().flush().ok();
    let mut line = String::new();
    reader.read_line(&mut line).unwrap_or(0);
    let trimmed = line.trim();
    if let Ok(idx) = trimmed.parse::<usize>()
        && idx >= 1
        && idx <= options.len()
    {
        return options[idx - 1].to_string();
    }
    // Try matching as text
    for opt in options {
        if opt.eq_ignore_ascii_case(trimmed) {
            return opt.to_string();
        }
    }
    eprintln!("Invalid selection: {trimmed}");
    std::process::exit(2);
}

fn prompt_input(reader: &mut impl BufRead, label: &str) -> String {
    eprint!("{label}: ");
    std::io::stderr().flush().ok();
    let mut line = String::new();
    reader.read_line(&mut line).unwrap_or(0);
    let trimmed = line.trim().to_string();
    if trimmed.is_empty() {
        eprintln!("No input provided");
        std::process::exit(2);
    }
    trimmed
}

fn generate_template(calc_type: &str, edge: &str, symbol: &str, z: u32) -> String {
    let mut lines = Vec::new();

    lines.push(format!("TITLE {symbol} {edge}-edge {calc_type}"));
    lines.push(format!("EDGE  {edge}"));
    lines.push("S02   1.0".to_string());
    lines.push(String::new());

    match calc_type {
        "EXAFS" => {
            lines.push("CONTROL 1 1 1 1 1 1".to_string());
            lines.push("PRINT   0 0 0 0 0 0".to_string());
            lines.push(String::new());
            lines.push("EXAFS  20.0".to_string());
            lines.push("RPATH  5.5".to_string());
        }
        "XANES" => {
            lines.push("CONTROL 1 1 1 1 1 1".to_string());
            lines.push("PRINT   0 0 0 0 0 0".to_string());
            lines.push(String::new());
            lines.push("SCF    5.0".to_string());
            lines.push("FMS    6.0".to_string());
            lines.push("XANES  4.0".to_string());
            lines.push("COREHOLE RPA".to_string());
        }
        "XES" => {
            lines.push("CONTROL 1 1 1 1 1 1".to_string());
            lines.push("PRINT   0 0 0 0 0 0".to_string());
            lines.push(String::new());
            lines.push("SCF    5.0".to_string());
            lines.push("FMS    6.0".to_string());
            lines.push("XES".to_string());
            lines.push("COREHOLE RPA".to_string());
        }
        _ => {
            lines.push("CONTROL 1 1 1 1 1 1".to_string());
            lines.push("PRINT   0 0 0 0 0 0".to_string());
        }
    }

    lines.push(String::new());
    lines.push("POTENTIALS".to_string());
    lines.push("*    ipot   Z   tag".to_string());
    lines.push(format!("      0    {z:<4}{symbol}"));
    lines.push(String::new());
    lines.push("ATOMS".to_string());
    lines.push("*    x        y        z     ipot  tag       distance".to_string());
    lines.push(format!(
        "     0.000    0.000    0.000  0    {symbol:<10}0.000"
    ));
    lines.push("* Add your cluster atoms here".to_string());
    lines.push(String::new());
    lines.push("END".to_string());
    lines.push(String::new());

    lines.join("\n")
}

fn cmd_stages(global: &GlobalArgs) -> CliResult {
    let stages = Stage::all();

    if global.json {
        let output: Vec<StageInfoOutput> = stages
            .iter()
            .enumerate()
            .map(|(i, s)| StageInfoOutput {
                name: s.executable_name().to_string(),
                control_group: s.control_index(),
                order: i + 1,
            })
            .collect();
        print_json(&output)?;
    } else if !global.quiet {
        println!("{:<6} {:<12} CONTROL group", "Order", "Stage");
        println!("{}", "-".repeat(32));
        for (i, stage) in stages.iter().enumerate() {
            println!(
                "{:>5}  {:<12} {}",
                i + 1,
                stage.executable_name(),
                stage.control_index()
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn parse_requested_stages_rejects_unknown() {
        let stages = vec!["rdinp".to_string(), "bad".to_string()];
        let err = parse_requested_stages(&stages).unwrap_err();
        assert!(err.contains("Unknown stage(s)"));
        assert!(err.contains("bad"));
    }

    #[test]
    fn parse_requested_stages_deduplicates() {
        let stages = vec!["pot".to_string(), "pot".to_string(), "fms".to_string()];
        let parsed = parse_requested_stages(&stages).unwrap();
        assert_eq!(parsed, vec![Stage::Pot, Stage::Fms]);
    }

    #[test]
    fn normalize_compare_columns_rejects_zero() {
        let err = normalize_compare_columns(0, 4).unwrap_err();
        assert!(err.contains(">= 1"));
    }

    #[test]
    fn normalize_compare_columns_converts_to_zero_based() {
        let cols = normalize_compare_columns(1, 4).unwrap();
        assert_eq!(cols, (0, 3));
    }

    #[test]
    fn cli_parses_global_quiet() {
        let cli = Cli::try_parse_from(["feff10-rs", "--quiet", "stages"]).unwrap();
        assert!(cli.global.quiet);
    }

    #[test]
    fn cli_parses_global_json() {
        let cli = Cli::try_parse_from(["feff10-rs", "--json", "stages"]).unwrap();
        assert!(cli.global.json);
    }

    #[test]
    fn cli_parses_global_verbose() {
        let cli = Cli::try_parse_from(["feff10-rs", "--verbose", "stages"]).unwrap();
        assert!(cli.global.verbose);
    }

    #[test]
    fn cli_parses_run_timeout() {
        let cli = Cli::try_parse_from(["feff10-rs", "run", ".", "--timeout", "30"]).unwrap();
        match cli.command {
            Command::Run { timeout, .. } => assert_eq!(timeout, Some(30)),
            _ => panic!("expected Run"),
        }
    }

    #[test]
    fn cli_parses_compare_threshold() {
        let cli = Cli::try_parse_from([
            "feff10-rs",
            "compare",
            "a.dat",
            "b.dat",
            "--threshold",
            "0.5",
        ])
        .unwrap();
        match cli.command {
            Command::Compare { threshold, .. } => assert!((threshold - 0.5).abs() < f64::EPSILON),
            _ => panic!("expected Compare"),
        }
    }

    #[test]
    fn cli_global_flags_work_after_subcommand() {
        let cli = Cli::try_parse_from(["feff10-rs", "stages", "--json"]).unwrap();
        assert!(cli.global.json);
    }

    #[test]
    fn cli_parses_examples_list() {
        let cli = Cli::try_parse_from(["feff10-rs", "examples"]).unwrap();
        match cli.command {
            Command::Examples { name, output } => {
                assert!(name.is_none());
                assert!(output.is_none());
            }
            _ => panic!("expected Examples"),
        }
    }

    #[test]
    fn cli_parses_examples_with_name_and_output() {
        let cli =
            Cli::try_parse_from(["feff10-rs", "examples", "exafs-cu", "-o", "/tmp/test"]).unwrap();
        match cli.command {
            Command::Examples { name, output } => {
                assert_eq!(name.as_deref(), Some("exafs-cu"));
                assert_eq!(output.as_deref(), Some(std::path::Path::new("/tmp/test")));
            }
            _ => panic!("expected Examples"),
        }
    }

    #[test]
    fn cli_parses_init_noninteractive() {
        let cli = Cli::try_parse_from([
            "feff10-rs",
            "init",
            "-t",
            "exafs",
            "--edge",
            "K",
            "--element",
            "Cu",
            "-o",
            "out.inp",
        ])
        .unwrap();
        match cli.command {
            Command::Init {
                calc_type,
                edge,
                element,
                output,
            } => {
                assert_eq!(calc_type.as_deref(), Some("exafs"));
                assert_eq!(edge.as_deref(), Some("K"));
                assert_eq!(element.as_deref(), Some("Cu"));
                assert_eq!(output.as_deref(), Some(std::path::Path::new("out.inp")));
            }
            _ => panic!("expected Init"),
        }
    }

    #[test]
    fn element_lookup_by_symbol() {
        let (sym, z) = element_symbol_to_z("Cu").unwrap();
        assert_eq!(sym, "Cu");
        assert_eq!(z, 29);
    }

    #[test]
    fn element_lookup_by_z() {
        let (sym, z) = element_symbol_to_z("29").unwrap();
        assert_eq!(sym, "Cu");
        assert_eq!(z, 29);
    }

    #[test]
    fn element_lookup_case_insensitive() {
        let (sym, z) = element_symbol_to_z("cu").unwrap();
        assert_eq!(sym, "Cu");
        assert_eq!(z, 29);
    }

    #[test]
    fn element_lookup_unknown() {
        assert!(element_symbol_to_z("Xx").is_none());
        assert!(element_symbol_to_z("999").is_none());
    }

    #[test]
    fn examples_all_non_empty() {
        for (name, desc, content) in EXAMPLES {
            assert!(!name.is_empty(), "example name is empty");
            assert!(!desc.is_empty(), "example description is empty for {name}");
            assert!(!content.is_empty(), "example content is empty for {name}");
            assert!(
                content.contains("ATOMS"),
                "example {name} missing ATOMS card"
            );
        }
    }

    #[test]
    fn generate_exafs_template() {
        let t = generate_template("EXAFS", "K", "Cu", 29);
        assert!(t.contains("TITLE Cu K-edge EXAFS"));
        assert!(t.contains("EDGE  K"));
        assert!(t.contains("EXAFS"));
        assert!(t.contains("RPATH"));
        assert!(t.contains("29"));
    }

    #[test]
    fn generate_xanes_template() {
        let t = generate_template("XANES", "L3", "Fe", 26);
        assert!(t.contains("TITLE Fe L3-edge XANES"));
        assert!(t.contains("SCF"));
        assert!(t.contains("FMS"));
        assert!(t.contains("XANES"));
        assert!(t.contains("COREHOLE"));
    }

    #[test]
    fn generate_xes_template() {
        let t = generate_template("XES", "K", "Mn", 25);
        assert!(t.contains("XES"));
        assert!(t.contains("COREHOLE"));
        assert!(t.contains("25"));
    }

    #[test]
    fn prompt_choice_parses_index() {
        let input = b"2\n";
        let mut reader = std::io::BufReader::new(&input[..]);
        let result = prompt_choice(&mut reader, "Test", &["A", "B", "C"]);
        assert_eq!(result, "B");
    }

    #[test]
    fn prompt_choice_parses_text() {
        let input = b"XANES\n";
        let mut reader = std::io::BufReader::new(&input[..]);
        let result = prompt_choice(&mut reader, "Type", &["EXAFS", "XANES", "XES"]);
        assert_eq!(result, "XANES");
    }

    #[test]
    fn prompt_input_reads_line() {
        let input = b"Cu\n";
        let mut reader = std::io::BufReader::new(&input[..]);
        let result = prompt_input(&mut reader, "Element");
        assert_eq!(result, "Cu");
    }

    // --- Additional CLI arg parsing tests ---

    #[test]
    fn cli_parses_run_default_input() {
        let cli = Cli::try_parse_from(["feff10-rs", "run"]).unwrap();
        match cli.command {
            Command::Run { input, .. } => assert_eq!(input, PathBuf::from(".")),
            _ => panic!("expected Run"),
        }
    }

    #[test]
    fn cli_parses_run_with_stages() {
        let cli = Cli::try_parse_from(["feff10-rs", "run", ".", "-s", "rdinp,pot,xsph"]).unwrap();
        match cli.command {
            Command::Run { stages, .. } => {
                let s = stages.unwrap();
                assert_eq!(s, vec!["rdinp", "pot", "xsph"]);
            }
            _ => panic!("expected Run"),
        }
    }

    #[test]
    fn cli_parses_run_with_work_dir() {
        let cli = Cli::try_parse_from(["feff10-rs", "run", "feff.inp", "-w", "/tmp/work"]).unwrap();
        match cli.command {
            Command::Run { work_dir, .. } => {
                assert_eq!(work_dir, Some(PathBuf::from("/tmp/work")));
            }
            _ => panic!("expected Run"),
        }
    }

    #[test]
    fn cli_parses_bench() {
        let cli = Cli::try_parse_from([
            "feff10-rs",
            "bench",
            "dir1",
            "dir2",
            "-n",
            "5",
            "-l",
            "test-label",
            "-o",
            "results.json",
        ])
        .unwrap();
        match cli.command {
            Command::Bench {
                inputs,
                iterations,
                label,
                output,
            } => {
                assert_eq!(inputs, vec![PathBuf::from("dir1"), PathBuf::from("dir2")]);
                assert_eq!(iterations, 5);
                assert_eq!(label, "test-label");
                assert_eq!(output, Some(PathBuf::from("results.json")));
            }
            _ => panic!("expected Bench"),
        }
    }

    #[test]
    fn cli_parses_compare_defaults() {
        let cli = Cli::try_parse_from(["feff10-rs", "compare", "a.dat", "b.dat"]).unwrap();
        match cli.command {
            Command::Compare {
                col_x,
                col_y,
                threshold,
                ..
            } => {
                assert_eq!(col_x, 1);
                assert_eq!(col_y, 4);
                assert!((threshold - 0.1).abs() < f64::EPSILON);
            }
            _ => panic!("expected Compare"),
        }
    }

    #[test]
    fn cli_parses_init_no_args() {
        let cli = Cli::try_parse_from(["feff10-rs", "init"]).unwrap();
        match cli.command {
            Command::Init {
                calc_type,
                edge,
                element,
                output,
            } => {
                assert!(calc_type.is_none());
                assert!(edge.is_none());
                assert!(element.is_none());
                assert!(output.is_none());
            }
            _ => panic!("expected Init"),
        }
    }

    #[test]
    fn cli_multiple_global_flags() {
        let cli = Cli::try_parse_from(["feff10-rs", "--quiet", "--json", "stages"]).unwrap();
        assert!(cli.global.quiet);
        assert!(cli.global.json);
        assert!(!cli.global.verbose);
    }

    #[test]
    fn cli_rejects_unknown_subcommand() {
        let result = Cli::try_parse_from(["feff10-rs", "nonexistent"]);
        assert!(result.is_err());
    }

    // --- Stage parsing edge cases ---

    #[test]
    fn parse_requested_stages_empty_input() {
        let stages: Vec<String> = Vec::new();
        let parsed = parse_requested_stages(&stages).unwrap();
        assert!(parsed.is_empty());
    }

    #[test]
    fn parse_requested_stages_single() {
        let stages = vec!["pot".to_string()];
        let parsed = parse_requested_stages(&stages).unwrap();
        assert_eq!(parsed, vec![Stage::Pot]);
    }

    #[test]
    fn parse_requested_stages_preserves_order() {
        let stages = vec!["ff2x".to_string(), "rdinp".to_string(), "pot".to_string()];
        let parsed = parse_requested_stages(&stages).unwrap();
        assert_eq!(parsed, vec![Stage::Ff2x, Stage::Rdinp, Stage::Pot]);
    }

    #[test]
    fn parse_requested_stages_all_18() {
        let all_names: Vec<String> = Stage::all()
            .iter()
            .map(|s| s.executable_name().to_string())
            .collect();
        let parsed = parse_requested_stages(&all_names).unwrap();
        assert_eq!(parsed.len(), 18);
    }

    // --- Column normalization ---

    #[test]
    fn normalize_compare_columns_both_zero() {
        let err = normalize_compare_columns(0, 0).unwrap_err();
        assert!(err.contains(">= 1"));
    }

    #[test]
    fn normalize_compare_columns_large_values() {
        let (x, y) = normalize_compare_columns(10, 20).unwrap();
        assert_eq!(x, 9);
        assert_eq!(y, 19);
    }

    // --- Element table tests ---

    #[test]
    fn element_lookup_hydrogen() {
        let (sym, z) = element_symbol_to_z("H").unwrap();
        assert_eq!(sym, "H");
        assert_eq!(z, 1);
    }

    #[test]
    fn element_lookup_fermium() {
        let (sym, z) = element_symbol_to_z("Fm").unwrap();
        assert_eq!(sym, "Fm");
        assert_eq!(z, 100);
    }

    #[test]
    fn element_lookup_by_z_boundaries() {
        let (sym, _) = element_symbol_to_z("1").unwrap();
        assert_eq!(sym, "H");
        let (sym, _) = element_symbol_to_z("100").unwrap();
        assert_eq!(sym, "Fm");
    }

    #[test]
    fn element_table_no_duplicate_symbols() {
        let mut seen = std::collections::HashSet::new();
        for (sym, _) in ELEMENTS {
            assert!(seen.insert(sym), "duplicate symbol: {sym}");
        }
    }

    #[test]
    fn element_table_no_duplicate_z() {
        let mut seen = std::collections::HashSet::new();
        for (_, z) in ELEMENTS {
            assert!(seen.insert(z), "duplicate Z: {z}");
        }
    }

    #[test]
    fn element_table_contiguous_z() {
        for (i, (_, z)) in ELEMENTS.iter().enumerate() {
            assert_eq!(
                *z as usize,
                i + 1,
                "element at index {i} has Z={z}, expected {}",
                i + 1
            );
        }
    }

    // --- Examples tests ---

    #[test]
    fn examples_unique_names() {
        let mut seen = std::collections::HashSet::new();
        for (name, _, _) in EXAMPLES {
            assert!(seen.insert(name), "duplicate example name: {name}");
        }
    }

    #[test]
    fn examples_parseable_by_feff_input() {
        for (name, _, content) in EXAMPLES {
            let result = FeffInput::parse(content);
            assert!(
                result.is_ok(),
                "example {name} failed to parse: {}",
                result.unwrap_err()
            );
            let input = result.unwrap();
            assert!(
                !input.potentials.is_empty(),
                "example {name} has no potentials"
            );
            assert!(!input.atoms.is_empty(), "example {name} has no atoms");
        }
    }

    #[test]
    fn examples_have_end_card() {
        for (name, _, content) in EXAMPLES {
            assert!(
                content
                    .lines()
                    .any(|l| l.trim().to_uppercase().starts_with("END")),
                "example {name} missing END card"
            );
        }
    }

    // --- Template tests ---

    #[test]
    fn generate_template_has_end() {
        for calc_type in &["EXAFS", "XANES", "XES"] {
            let t = generate_template(calc_type, "K", "Cu", 29);
            assert!(t.contains("END"), "{calc_type} template missing END");
        }
    }

    #[test]
    fn generate_template_parseable() {
        for calc_type in &["EXAFS", "XANES", "XES"] {
            let t = generate_template(calc_type, "K", "Cu", 29);
            let result = FeffInput::parse(&t);
            assert!(
                result.is_ok(),
                "{calc_type} template failed to parse: {}",
                result.unwrap_err()
            );
            let input = result.unwrap();
            assert_eq!(input.edge.as_deref(), Some("K"));
            assert!(!input.potentials.is_empty());
            assert!(!input.atoms.is_empty());
        }
    }

    #[test]
    fn generate_template_unknown_type_still_valid() {
        let t = generate_template("UNKNOWN", "K", "Fe", 26);
        assert!(t.contains("CONTROL"));
        assert!(t.contains("POTENTIALS"));
        assert!(t.contains("ATOMS"));
        assert!(t.contains("END"));
        let input = FeffInput::parse(&t).unwrap();
        assert!(!input.potentials.is_empty());
    }

    #[test]
    fn generate_template_various_elements() {
        for (sym, z) in &[("H", 1u32), ("Fe", 26), ("U", 92)] {
            let t = generate_template("EXAFS", "K", sym, *z);
            assert!(t.contains(&format!("{z}")));
            assert!(t.contains(sym));
        }
    }

    #[test]
    fn generate_template_various_edges() {
        for edge in &["K", "L1", "L2", "L3"] {
            let t = generate_template("XANES", edge, "Cu", 29);
            assert!(t.contains(&format!("EDGE  {edge}")));
            assert!(t.contains(&format!("{edge}-edge")));
        }
    }

    // --- Functional tests ---

    #[test]
    fn cmd_examples_writes_file_to_tmpdir() {
        let tmp = tempfile::tempdir().unwrap();
        let global = GlobalArgs {
            quiet: true,
            verbose: false,
            json: false,
        };
        cmd_examples(
            Some("exafs-sf6".to_string()),
            Some(tmp.path().to_path_buf()),
            &global,
        )
        .expect("cmd_examples should succeed");
        let feff_inp = tmp.path().join("feff.inp");
        assert!(feff_inp.exists(), "feff.inp not created");
        let content = std::fs::read_to_string(&feff_inp).unwrap();
        assert!(content.contains("SF6"));
    }

    #[test]
    fn cmd_examples_each_example_writes_correctly() {
        let global = GlobalArgs {
            quiet: true,
            verbose: false,
            json: false,
        };
        for (name, _, expected_content) in EXAMPLES {
            let tmp = tempfile::tempdir().unwrap();
            cmd_examples(
                Some(name.to_string()),
                Some(tmp.path().to_path_buf()),
                &global,
            )
            .expect("cmd_examples should succeed");
            let content = std::fs::read_to_string(tmp.path().join("feff.inp")).unwrap();
            assert_eq!(content, *expected_content, "mismatch for example {name}");
        }
    }

    // --- Prompt tests ---

    #[test]
    fn prompt_choice_case_insensitive_text() {
        let input = b"xanes\n";
        let mut reader = std::io::BufReader::new(&input[..]);
        let result = prompt_choice(&mut reader, "Type", &["EXAFS", "XANES", "XES"]);
        assert_eq!(result, "XANES");
    }

    #[test]
    fn prompt_choice_first_and_last_index() {
        let input = b"1\n";
        let mut reader = std::io::BufReader::new(&input[..]);
        let result = prompt_choice(&mut reader, "Test", &["A", "B", "C"]);
        assert_eq!(result, "A");

        let input = b"3\n";
        let mut reader = std::io::BufReader::new(&input[..]);
        let result = prompt_choice(&mut reader, "Test", &["A", "B", "C"]);
        assert_eq!(result, "C");
    }

    #[test]
    fn prompt_input_trims_whitespace() {
        let input = b"  Cu  \n";
        let mut reader = std::io::BufReader::new(&input[..]);
        let result = prompt_input(&mut reader, "Element");
        assert_eq!(result, "Cu");
    }
}
