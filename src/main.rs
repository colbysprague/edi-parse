use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use clap::{Parser, Subcommand};
use edi_parse::{parse_str, process_dir, to_json, EdiError, Validate};

/// Parse, validate, and summarize X12 835/837P EDI files.
#[derive(Debug, Parser)]
#[command(name = "edi-parse", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Parse one file and print it as JSON.
    Parse {
        file: PathBuf,
        /// Pretty-print the JSON.
        #[arg(long)]
        pretty: bool,
    },
    /// Parse one file and report validation errors (exit code 1 if any).
    Validate { file: PathBuf },
    /// Process every .edi/.x12 file in a directory and print summary JSON.
    Summary {
        dir: PathBuf,
        /// Process files one at a time instead of in parallel.
        #[arg(long)]
        sequential: bool,
        /// How many top procedure codes to print to stderr.
        #[arg(long, default_value_t = 5)]
        top: usize,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Parse { file, pretty } => cmd_parse(&file, pretty),
        Command::Validate { file } => cmd_validate(&file),
        Command::Summary {
            dir,
            sequential,
            top,
        } => cmd_summary(&dir, !sequential, top),
    };
    match result {
        Ok(code) => code,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}

fn read(path: &Path) -> Result<String, EdiError> {
    std::fs::read_to_string(path).map_err(|source| EdiError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn cmd_parse(file: &Path, pretty: bool) -> Result<ExitCode, Box<dyn std::error::Error>> {
    let input = read(file)?;
    let interchange = parse_str(&input)?;
    println!("{}", to_json(&interchange, pretty)?);
    Ok(ExitCode::SUCCESS)
}

fn cmd_validate(file: &Path) -> Result<ExitCode, Box<dyn std::error::Error>> {
    let input = read(file)?;
    let interchange = parse_str(&input)?;
    match interchange.validate() {
        Ok(()) => {
            println!("{}: ok", file.display());
            Ok(ExitCode::SUCCESS)
        }
        Err(errors) => {
            println!("{}: {} validation error(s)", file.display(), errors.len());
            for error in &errors {
                println!("  - {error}");
            }
            Ok(ExitCode::FAILURE)
        }
    }
}

fn cmd_summary(
    dir: &Path,
    parallel: bool,
    top: usize,
) -> Result<ExitCode, Box<dyn std::error::Error>> {
    let start = Instant::now();
    let summary = process_dir(dir, parallel)?;
    let elapsed = start.elapsed();

    println!("{}", serde_json::to_string_pretty(&summary)?);

    eprintln!(
        "{} files ok, {} failed in {:.2?} ({})",
        summary.files_processed,
        summary.files_failed,
        elapsed,
        if parallel { "parallel" } else { "sequential" },
    );
    eprintln!(
        "{} claims billed ${:.2}; {} remits paid ${:.2}; {} validation error(s)",
        summary.claims,
        summary.total_billed,
        summary.remits,
        summary.total_paid,
        summary.validation_errors,
    );
    for (code, count) in summary.top_procedures(top) {
        eprintln!("  {code:<8} {count}");
    }
    Ok(ExitCode::SUCCESS)
}
