use std::{path::PathBuf, process::ExitCode};

use anyhow::Result;
use clap::{Parser, Subcommand};
use veoveo_installer::{hub, inputs::Inputs, run};

/// Makes a cluster match this installation repository.
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// The installation repository.
    #[arg(long, default_value = ".")]
    repository: PathBuf,
    /// The operator's private directory (mode 0700) holding private.toml.
    #[arg(long, env = "VEOVEO_INSTALLER_PRIVATE")]
    private: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Show, in apply's order, what differs between this repository and what runs, and
    /// what could not be read. Changes nothing. Exits 0 when everything was read and
    /// matches, 2 otherwise.
    Plan {
        /// Plan only this step; repeat for several.
        #[arg(long = "step")]
        steps: Vec<String>,
    },
    /// Sign in to Hub through the browser as an installation administrator, so plan
    /// can read Hub's control plane. The grant is stored where the Hub SDK CLI keeps it.
    Login {
        /// Print the sign-in address instead of opening a browser.
        #[arg(long)]
        no_browser: bool,
    },
}

#[tokio::main]
async fn main() -> ExitCode {
    let _ = rustls::crypto::ring::default_provider().install_default();
    match run(Cli::parse()).await {
        Ok(code) => code,
        Err(error) => {
            eprintln!("veoveo-installer: {error:#}");
            ExitCode::FAILURE
        }
    }
}

async fn run(cli: Cli) -> Result<ExitCode> {
    let inputs = Inputs::read(&cli.repository, &cli.private)?;
    match cli.command {
        Command::Plan { steps } => {
            let plan = run::plan(&inputs, &steps).await?;
            print!("{}", plan.render());
            Ok(if plan.matches() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(2)
            })
        }
        Command::Login { no_browser } => {
            hub::login(&inputs, no_browser).await?;
            Ok(ExitCode::SUCCESS)
        }
    }
}
