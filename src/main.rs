use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use clap::{Parser, Subcommand};
use edi_parse::{
    parse_dir, parse_str, parse_to_file, process_dir, process_dir_with_log, to_json, write_summary,
    EdiError, Mode, Summary, Validate, DEFAULT_READERS, VALIDATION_LOG_FILE,
};

/// Parse, validate, and summarize X12 835/837P EDI files.
#[derive(Debug, Parser)]
#[command(name = "edi-parse", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Parse a file and print it as JSON, or a directory (recursively) into --out.
    Parse {
        path: PathBuf,
        /// Pretty-print the JSON.
        #[arg(long)]
        pretty: bool,
        /// Write .json files to this directory, mirroring the input layout.
        /// Required when PATH is a directory.
        #[arg(long, short)]
        out: Option<PathBuf>,
    },
    /// Parse one file and report validation errors (exit code 1 if any).
    Validate { file: PathBuf },
    /// Process every .edi/.x12 file in a directory and print summary JSON.
    Summary {
        dir: PathBuf,
        /// Process files one at a time instead of in parallel.
        #[arg(long)]
        sequential: bool,
        /// How many threads read files in a parallel run.
        #[arg(long, default_value_t = DEFAULT_READERS, conflicts_with = "sequential")]
        readers: NonZeroUsize,
        /// How many top procedure codes to print to stderr.
        #[arg(long, default_value_t = 5)]
        top: usize,
        /// Write summary.json and validation_errors.json to this directory,
        /// instead of printing the summary to stdout.
        #[arg(long, short)]
        out: Option<PathBuf>,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Parse { path, pretty, out } => cmd_parse(&path, out.as_deref(), pretty),
        Command::Validate { file } => cmd_validate(&file),
        Command::Summary {
            dir,
            sequential,
            readers,
            top,
            out,
        } => {
            let mode = if sequential {
                Mode::Sequential
            } else {
                Mode::Parallel { readers }
            };
            cmd_summary(&dir, mode, top, out.as_deref())
        }
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

fn cmd_parse(
    path: &Path,
    out: Option<&Path>,
    pretty: bool,
) -> Result<ExitCode, Box<dyn std::error::Error>> {
    if path.is_dir() {
        let out = out.ok_or("--out is required when parsing a directory")?;
        return cmd_parse_dir(path, out, pretty);
    }
    if let Some(out) = out {
        let name = path.file_name().ok_or("input path has no file name")?;
        parse_to_file(path, &out.join(name).with_extension("json"), pretty)?;
        return Ok(ExitCode::SUCCESS);
    }

    let input = read(path)?;
    let interchange = parse_str(&input)?;
    println!("{}", to_json(&interchange, pretty)?);
    Ok(ExitCode::SUCCESS)
}

fn cmd_parse_dir(
    dir: &Path,
    out: &Path,
    pretty: bool,
) -> Result<ExitCode, Box<dyn std::error::Error>> {
    let report = parse_dir(dir, out, pretty)?;
    let summary_path = write_summary(&report.summary, out)?;
    for failure in &report.failed {
        eprintln!("error: {failure}");
    }
    eprintln!(
        "{} files ok, {} failed; JSON written to {}",
        report.written,
        report.failed.len(),
        out.display(),
    );
    eprintln!("summary written to {}", summary_path.display());
    report_validation_log(&report.summary, out);
    if report.failed.is_empty() {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::FAILURE)
    }
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

fn report_validation_log(summary: &Summary, out: &Path) {
    if summary.validation_errors > 0 {
        eprintln!(
            "{} validation error(s) written to {}",
            summary.validation_errors,
            out.join(VALIDATION_LOG_FILE).display(),
        );
    }
}

fn cmd_summary(
    dir: &Path,
    mode: Mode,
    top: usize,
    out: Option<&Path>,
) -> Result<ExitCode, Box<dyn std::error::Error>> {
    let start = Instant::now();
    let summary = match out {
        Some(out) => process_dir_with_log(dir, mode, out)?,
        None => process_dir(dir, mode)?,
    };
    let elapsed = start.elapsed();

    match out {
        Some(out) => {
            let dest = write_summary(&summary, out)?;
            eprintln!("summary written to {}", dest.display());
            report_validation_log(&summary, out);
        }
        None => {
            println!("{}", serde_json::to_string_pretty(&summary)?);
            if summary.validation_errors > 0 {
                eprintln!("use --out to write each validation error to {VALIDATION_LOG_FILE}");
            }
        }
    }

    eprintln!(
        "{} files ok, {} failed in {:.2?} ({})",
        summary.files_processed,
        summary.files_failed,
        elapsed,
        match mode {
            Mode::Sequential => "sequential",
            Mode::Parallel { .. } => "parallel",
        },
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
