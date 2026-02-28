use std::collections::BTreeMap;
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

#[derive(Parser)]
#[command(name = "feff10", about = "FEFF10 X-ray absorption spectroscopy calculations")]
enum Cli {
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
}

fn main() {
    let cli = Cli::parse();

    match cli {
        Cli::Run {
            input,
            work_dir,
            stages,
        } => cmd_run(input, work_dir, stages),
        Cli::Validate { input } => cmd_validate(input),
        Cli::Compare {
            file1,
            file2,
            col_x,
            col_y,
        } => cmd_compare(file1, file2, col_x, col_y),
        Cli::Bench {
            inputs,
            iterations,
            output,
            label,
        } => cmd_bench(inputs, iterations, output, label),
    }
}

fn cmd_run(input: PathBuf, work_dir: Option<PathBuf>, stage_names: Option<Vec<String>>) {
    // Resolve input path
    let inp_path = if input.is_dir() {
        input.join("feff.inp")
    } else {
        input.clone()
    };

    if !inp_path.exists() {
        eprintln!("Error: {} not found", inp_path.display());
        std::process::exit(1);
    }

    // Resolve working directory
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
            std::process::exit(1);
        }
    };

    let mut builder = FeffConfigBuilder::new().work_dir(&work).input(feff_input);

    if let Some(names) = stage_names {
        let stages: Vec<Stage> = names
            .iter()
            .filter_map(|name| {
                Stage::default_pipeline()
                    .into_iter()
                    .find(|s| s.executable_name() == name.as_str())
            })
            .collect();
        builder = builder.stages(stages);
    }

    let config = match builder.build() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Config error: {e}");
            std::process::exit(1);
        }
    };

    let total = config.stages.len() as u64;
    let pb = ProgressBar::new(total);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {bar:40} {pos}/{len} {msg}")
            .unwrap(),
    );

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
            println!("FEFF calculation completed successfully in {}", res.work_dir.display());
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
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }
}

fn cmd_validate(input: PathBuf) {
    match FeffInput::from_file(&input) {
        Ok(inp) => {
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
            let active: Vec<_> = Stage::default_pipeline()
                .iter()
                .filter(|s| inp.control[s.control_index()] != 0)
                .map(|s| s.executable_name())
                .collect();
            println!("  Active stages: {}", active.join(", "));
        }
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }
}

fn cmd_compare(file1: PathBuf, file2: PathBuf, col_x: usize, col_y: usize) {
    let xmu1 = match XmuDat::from_file(&file1) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("Error reading {}: {e}", file1.display());
            std::process::exit(1);
        }
    };
    let xmu2 = match XmuDat::from_file(&file2) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("Error reading {}: {e}", file2.display());
            std::process::exit(1);
        }
    };

    // Convert from 1-based to 0-based
    let rsq = xmu1.r_squared(&xmu2, col_x - 1, col_y - 1);
    let pct = rsq * 100.0;
    println!(
        "R-squared comparison (columns {col_x} vs {col_y}):"
    );
    println!("  {} vs {}", file1.display(), file2.display());
    println!("  Average deviation: {pct:.6}%");
    if pct < 0.1 {
        println!("  Result: PASS (< 0.1%)");
    } else {
        println!("  Result: FAIL (>= 0.1%)");
        std::process::exit(1);
    }
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

fn cmd_bench(inputs: Vec<PathBuf>, iterations: usize, output: Option<PathBuf>, label: String) {
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
                // Try to get a nice name like "EXAFS/Cu" from the path
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

        eprintln!("Benchmarking {example_name} ({iterations} iterations)...");

        let mut stage_timings: BTreeMap<String, Vec<f64>> = BTreeMap::new();
        let mut total_times: Vec<f64> = Vec::new();

        for iter in 0..iterations {
            let work_dir = tempfile::tempdir().unwrap();
            std::fs::copy(&inp_path, work_dir.path().join("feff.inp")).unwrap();

            let feff_input = FeffInput::from_file(work_dir.path().join("feff.inp")).unwrap();
            let config = FeffConfigBuilder::new()
                .work_dir(work_dir.path())
                .input(feff_input)
                .build()
                .unwrap();

            let pipeline = FeffPipeline::new(config);
            match pipeline.run() {
                Ok(res) => {
                    let mut total = 0.0;
                    for sr in &res.stages {
                        let secs = sr.duration.as_secs_f64();
                        stage_timings
                            .entry(sr.stage.executable_name().to_string())
                            .or_default()
                            .push(secs);
                        total += secs;
                    }
                    total_times.push(total);
                    eprintln!(
                        "  iteration {}/{}: {:.2}s",
                        iter + 1,
                        iterations,
                        total
                    );
                }
                Err(e) => {
                    eprintln!("  iteration {}/{}: FAILED - {e}", iter + 1, iterations);
                    // Push NaN so the iteration count stays correct
                    total_times.push(f64::NAN);
                }
            }
        }

        let valid_totals: Vec<f64> = total_times.iter().copied().filter(|t| t.is_finite()).collect();
        let mean = if valid_totals.is_empty() {
            f64::NAN
        } else {
            valid_totals.iter().sum::<f64>() / valid_totals.len() as f64
        };
        let min = valid_totals.iter().copied().fold(f64::INFINITY, f64::min);
        let max = valid_totals.iter().copied().fold(f64::NEG_INFINITY, f64::max);

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

    // Print summary table
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

    // Per-stage breakdown for each example
    for ex in &report.examples {
        println!("--- {} stage breakdown (mean) ---", ex.name);
        for (stage, mean) in &ex.stage_means {
            println!("  {stage:>12}: {mean:.3}s");
        }
        println!();
    }

    // Write JSON if requested
    if let Some(out_path) = output {
        let json = serde_json::to_string_pretty(&report).unwrap();
        std::fs::write(&out_path, &json).unwrap();
        eprintln!("JSON results written to {}", out_path.display());
    }
}
