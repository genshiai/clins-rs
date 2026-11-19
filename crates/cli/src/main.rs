// SPDX-License-Identifier: Apache-2.0
use anyhow::{Context, Result, ensure};
use clap::{Parser, Subcommand};
use clins_core::MAX_INPUT_BYTES;
use std::{io::Read, net::SocketAddr, path::PathBuf, process::ExitCode};

#[derive(Parser)]
#[command(
    version,
    about = "Native Rust FHIR R4 / JP Core / CLINS validator (see coverage)"
)]
struct Cli {
    /// Local R4 StructureDefinition JSON, repeatable; no network downloads.
    #[arg(long, global = true)]
    definition: Vec<PathBuf>,
    /// Checksum-pinned local tgz package manifest; mutually exclusive with --definition.
    #[arg(long, global = true, conflicts_with = "definition")]
    packages: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Download and verify the public assets in --packages; never sends patient data.
    Prepare,
    /// Export verified public definitions for Python, WASM and Kotlin.
    ExportDefinitions,
    /// Write a validation report. Exit 0 valid, 1 invalid, 2 error, 3 incomplete.
    Validate {
        /// FHIR resource file; omit to read stdin.
        file: Option<PathBuf>,
        #[arg(long)]
        profile: Option<String>,
    },
    /// Serve POST /validate. Binds to loopback by default; no authentication.
    Serve {
        #[arg(long, default_value = "127.0.0.1:8091")]
        bind: SocketAddr,
        #[arg(long, default_value = "2")]
        concurrency: std::num::NonZeroUsize,
    },
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("{error:#}");
            ExitCode::from(2)
        }
    }
}

async fn run() -> Result<u8> {
    let cli = Cli::parse();
    if matches!(cli.command, Command::Prepare) {
        let path = cli.packages.context("prepare requires --packages")?;
        tokio::task::spawn_blocking(move || clins_packages::prepare(&path)).await??;
        return Ok(0);
    }
    if matches!(cli.command, Command::ExportDefinitions) {
        let path = cli
            .packages
            .context("export-definitions requires --packages")?;
        let (packages, resources) = clins_packages::load(&path)?;
        println!(
            "{}",
            serde_json::to_string(
                &serde_json::json!({"packages": packages, "resources": resources})
            )?
        );
        return Ok(0);
    }
    let service = match cli.packages {
        Some(path) => clins_packages::from_lock(&path)?,
        None => clins_packages::from_definitions(&cli.definition)?,
    };
    match cli.command {
        Command::Prepare | Command::ExportDefinitions => unreachable!("handled above"),
        Command::Validate { file, profile } => {
            let reader: Box<dyn Read> = match file {
                Some(path) => Box::new(std::fs::File::open(path).context("cannot open resource")?),
                None => Box::new(std::io::stdin()),
            };
            let mut data = Vec::new();
            reader
                .take((MAX_INPUT_BYTES + 1) as u64)
                .read_to_end(&mut data)?;
            ensure!(
                data.len() <= MAX_INPUT_BYTES,
                "resource exceeds 2 MiB limit"
            );
            let resource = std::str::from_utf8(&data).context("input must be UTF-8 JSON")?;
            let report = service.validate_json(resource, profile)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
            Ok(match report.status {
                "valid" => 0,
                "invalid" => 1,
                _ => 3,
            })
        }
        Command::Serve { bind, concurrency } => {
            let listener = tokio::net::TcpListener::bind(bind).await?;
            eprintln!(
                "Offline Rust FHIR validation on http://{}",
                listener.local_addr()?
            );
            axum::serve(
                listener,
                clins_server::http::router(service, concurrency.get()),
            )
            .with_graceful_shutdown(async {
                let _ = tokio::signal::ctrl_c().await;
            })
            .await?;
            Ok(0)
        }
    }
}
