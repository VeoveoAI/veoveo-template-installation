use std::{path::PathBuf, process::ExitCode};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use veoveo_installer::{
    apply, hub,
    identity::{self, keycloak},
    inputs::Inputs,
    run,
};

/// Makes a cluster match this installation repository.
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// The installation repository.
    #[arg(long, default_value = ".")]
    repository: PathBuf,
    /// The operator's private directory (mode 0700) holding private.toml; the Mac
    /// fixtures keep theirs in the repository's ignored state/ instead.
    #[arg(long, env = "VEOVEO_INSTALLER_PRIVATE")]
    private: Option<PathBuf>,
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
    /// Make exactly the plan's changes, step by step, checking each, and stop at the first
    /// failure with its evidence. Changes nothing unless the plan read every piece and
    /// has a change for every difference. Run again, it changes nothing.
    Apply {
        /// Apply only this step; repeat for several.
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
    /// Prepare installation identity.
    #[command(subcommand)]
    Identity(IdentityCommand),
    /// Prepare the Mac development fixtures of local Keycloak, OpenShell and Hub.
    #[command(subcommand)]
    Fixture(FixtureCommand),
}

#[derive(Subcommand)]
enum IdentityCommand {
    /// Prepare Hub's trusted files in the private directory from the identity
    /// provider's registration: hub.env with Hub's keys, made once and never replaced,
    /// the authorization server's key set and the seed hub.json.
    Hub {
        /// Verify the existing files and say what would be written; write nothing.
        #[arg(long)]
        check: bool,
    },
}

#[derive(Subcommand)]
enum FixtureCommand {
    /// Provision the persistent local Keycloak with its database, TLS edge, permanent
    /// administrator and realm.
    Keycloak {
        /// Renew only the identity TLS leaf under its existing CA and key.
        #[arg(long)]
        renew_identity_certificate: bool,
    },
    /// Configure the local OpenShell gateway's OIDC clients.
    Openshell,
    /// Prepare the fixture Hub's catalog, keys and local Keycloak service clients.
    Hub {
        #[arg(long, default_value = keycloak::HUB_ORIGIN)]
        hub_origin: String,
        /// Bind the private vv-services/artifact server.
        #[arg(long)]
        include_artifact: bool,
        /// Bind vv-services/media, which needs Artifact.
        #[arg(long)]
        include_media: bool,
        /// Bind vv-services/embedding, adding no role or billed risk.
        #[arg(long)]
        include_embedding: bool,
    },
    /// Configure only a local human PKCE client and write a catalog candidate, which is
    /// published separately through Hub's administrative route.
    PublicClient {
        /// A fresh snapshot of the active control plane.
        #[arg(long)]
        control_plane_file: PathBuf,
        /// The private candidate to write.
        #[arg(long)]
        output_control_plane: PathBuf,
        /// The existing MCP resource of the client.
        #[arg(long)]
        protected_resource: String,
        #[arg(long)]
        work_context: String,
        #[arg(long = "member-object-id", required = true)]
        members: Vec<String>,
        #[arg(long = "client-redirect-uri", required = true)]
        redirects: Vec<String>,
        #[arg(long, default_value = "opencode-local")]
        application_client_id: String,
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
    let inputs = || -> Result<Inputs> {
        let private = cli
            .private
            .as_deref()
            .context("this command needs --private, the operator's private directory")?;
        Inputs::read(&cli.repository, private)
    };
    match &cli.command {
        Command::Plan { steps } => {
            let (plan, _) = run::plan(&inputs()?, steps).await?;
            print!("{}", plan.render());
            Ok(if plan.matches() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(2)
            })
        }
        Command::Apply { steps } => {
            apply::apply(&inputs()?, steps).await?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Login { no_browser } => {
            hub::login(&inputs()?, *no_browser).await?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Identity(IdentityCommand::Hub { check }) => {
            let report = identity::hub::prepare(&inputs()?, *check)?;
            for line in report.lines {
                println!("{line}");
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Fixture(fixture) => {
            fixture_command(&cli.repository, fixture).await?;
            Ok(ExitCode::SUCCESS)
        }
    }
}

async fn fixture_command(repository: &std::path::Path, command: &FixtureCommand) -> Result<()> {
    match command {
        FixtureCommand::Keycloak {
            renew_identity_certificate,
        } => keycloak::provision(repository, *renew_identity_certificate).await,
        FixtureCommand::Openshell => keycloak::configure_openshell(repository).await,
        FixtureCommand::Hub {
            hub_origin,
            include_artifact,
            include_media,
            include_embedding,
        } => {
            keycloak::configure_hub(
                repository,
                hub_origin,
                *include_artifact,
                *include_media,
                *include_embedding,
            )
            .await
        }
        FixtureCommand::PublicClient {
            control_plane_file,
            output_control_plane,
            protected_resource,
            work_context,
            members,
            redirects,
            application_client_id,
        } => {
            keycloak::prepare_public_client(
                repository,
                control_plane_file,
                output_control_plane,
                &keycloak::PublicClient {
                    client_id: application_client_id.clone(),
                    members: members.clone(),
                    context: work_context.clone(),
                    resource: protected_resource.clone(),
                    redirects: redirects.clone(),
                },
            )
            .await
        }
    }
}
