//! What `apply` does about one planned difference, through the owner of the piece. A
//! plan carries the action of each difference, so `apply` makes exactly the plan's
//! changes and `plan` shows them.

use openshell_sdk::raw::proto;
use serde_json::Value;
use url::Url;

use crate::declaration::{HelmRelease, HubClaim, Service};

#[derive(Debug, Clone)]
pub enum Action {
    /// Server-side apply as `veoveo-installer`, then the object's rollout. `remove` are
    /// merge patches deleting members an earlier `kubectl apply` set that the declaration
    /// no longer has; `restart` the workloads that read the object only when they start.
    ApplyObject {
        object: Value,
        remove: Vec<Value>,
        restart: Vec<String>,
    },
    /// `helm upgrade --install` with the declared values and `--wait`.
    UpgradeRelease {
        release: HelmRelease,
        values: Vec<ValuesFile>,
    },
    CreateWorkspace(String),
    /// Lint, then import a profile that does not exist.
    ImportProfile {
        workspace: Option<String>,
        profile: Box<proto::ProviderProfile>,
        source: String,
    },
    /// Lint, then update a profile at the version it was read.
    UpdateProfile {
        workspace: Option<String>,
        profile: Box<proto::ProviderProfile>,
        source: String,
        resource_version: u64,
    },
    /// OpenShell has no template update: delete the template if it exists and create it.
    ReplaceTemplate {
        workspace: String,
        template: Box<proto::SandboxWorkloadTemplate>,
        exists: bool,
    },
    /// OpenShell has no sandbox update: stop and delete it if it exists, keeping its
    /// workspace, then create it, expose its services and wait until it is ready.
    ReplaceSandbox {
        workspace: String,
        name: String,
        request: Box<proto::CreateSandboxRequest>,
        services: Vec<Service>,
        exists: bool,
    },
    /// Expose a service, deleting a differing exposure of the same name first.
    ExposeService {
        workspace: String,
        sandbox: String,
        service: Service,
        exists: bool,
    },
    /// Write files into Hub's trusted claim with its seed Job, from a temporary Secret.
    WriteClaimFiles {
        claim: HubClaim,
        files: Vec<(String, String)>,
    },
    /// Publish the declared members through Hub's administrative route, keeping the
    /// members people change from the revision active at that moment, and restart Hub
    /// when a member it reads only at its start changed.
    Publish {
        origin: Url,
        profile: String,
        declared: Value,
        restart_hub: Option<(String, String)>,
    },
    /// Wait until Hub reports the server's deployment healthy.
    AwaitRegistration {
        origin: Url,
        profile: String,
        server: String,
        deployment: String,
    },
    /// Remove files of Hub's claim that the new active revision does not name.
    RemoveClaimFiles {
        claim: HubClaim,
        names: Vec<String>,
    },
}

/// One values file of a release: a repository file filled from the lock, or a private
/// file passed by its path.
#[derive(Debug, Clone)]
pub enum ValuesFile {
    Repository { name: String, text: String },
    Private { path: std::path::PathBuf },
}

impl Action {
    /// What apply does, in a line.
    pub fn describe(&self) -> String {
        match self {
            Self::ApplyObject {
                remove, restart, ..
            } => {
                let mut text = "server-side apply as veoveo-installer, then wait for it".to_owned();
                if !remove.is_empty() {
                    text.push_str(&format!(", removing {} member(s) no longer declared", remove.len()));
                }
                if !restart.is_empty() {
                    text.push_str(&format!("; restart {}", restart.join(", ")));
                }
                text
            }
            Self::UpgradeRelease { release, .. } => format!(
                "helm upgrade --install {} with the declared values and --wait",
                release.release
            ),
            Self::CreateWorkspace(name) => format!("create workspace {name}"),
            Self::ImportProfile { .. } => "lint and import the profile".to_owned(),
            Self::UpdateProfile {
                resource_version, ..
            } => format!("lint and update the profile at version {resource_version}"),
            Self::ReplaceTemplate { exists: true, .. } => {
                "delete the template and create it again, keeping its release note".to_owned()
            }
            Self::ReplaceTemplate { exists: false, .. } => "create the template".to_owned(),
            Self::ReplaceSandbox { exists: true, .. } => {
                "stop and delete the sandbox, keeping its workspace and claims, create it again and wait until it is ready"
                    .to_owned()
            }
            Self::ReplaceSandbox { exists: false, .. } => {
                "create the sandbox and wait until it is ready".to_owned()
            }
            Self::ExposeService { exists: true, .. } => {
                "delete the exposure and expose the service again".to_owned()
            }
            Self::ExposeService { exists: false, .. } => "expose the service".to_owned(),
            Self::WriteClaimFiles { claim, files } => format!(
                "write {} with seed Job {}",
                files
                    .iter()
                    .map(|(name, _)| name.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
                claim.job
            ),
            Self::Publish { restart_hub, .. } => {
                let mut text = "publish these members through the administrative route".to_owned();
                if let Some((workspace, name)) = restart_hub {
                    text.push_str(&format!(", then restart {workspace}/{name}, which reads them at its start"));
                }
                text
            }
            Self::AwaitRegistration { server, .. } => {
                format!("wait until Hub reports {server} healthy")
            }
            Self::RemoveClaimFiles { names, .. } => format!(
                "remove {} once the new revision is active",
                names.join(", ")
            ),
        }
    }
}
