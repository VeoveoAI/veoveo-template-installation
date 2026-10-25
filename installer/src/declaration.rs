//! `installation.toml`: the substrate this repository declares, as an ordered list of
//! steps. Each step changes through one owner's interface: Kubernetes objects, a Helm
//! release, OpenShell workspaces, provider profiles, templates or sandboxes, or Hub's
//! control plane. The order of the steps is the order `apply` takes.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;

/// The file at the repository's root.
pub const DECLARATION_FILE: &str = "installation.toml";

/// The declaration as written.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeclarationFile {
    #[serde(rename = "step")]
    steps: Vec<StepFile>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StepFile {
    name: String,
    #[serde(default)]
    manifests: Vec<PathBuf>,
    kustomize: Option<PathBuf>,
    #[serde(default, rename = "config_map")]
    config_maps: Vec<ConfigMap>,
    #[serde(default)]
    restart: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    after_hub: Vec<String>,
    helm: Option<HelmRelease>,
    #[serde(default)]
    workspaces: Vec<String>,
    #[serde(default, rename = "profile")]
    profiles: Vec<Profile>,
    #[serde(default, rename = "template")]
    templates: Vec<Template>,
    #[serde(default, rename = "sandbox")]
    sandboxes: Vec<Sandbox>,
    claim: Option<HubClaim>,
    hub: Option<Hub>,
}

/// The validated declaration.
#[derive(Debug, Clone)]
pub struct Declaration {
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone)]
pub struct Step {
    pub name: String,
    pub kind: StepKind,
}

/// What one step declares. A step holds one kind of thing, so it changes through one
/// owner's interface.
#[derive(Debug, Clone)]
pub enum StepKind {
    Kubernetes(KubernetesObjects),
    Helm(HelmRelease),
    Workspaces(Vec<String>),
    Profiles(Vec<Profile>),
    Templates(Vec<Template>),
    Sandboxes(Vec<Sandbox>),
    Claim(HubClaim),
    Hub(Hub),
}

impl StepKind {
    /// The owner whose interface the step uses.
    pub fn owner(&self) -> &'static str {
        match self {
            Self::Kubernetes(_) => "Kubernetes",
            Self::Helm(_) => "Helm",
            Self::Workspaces(_) | Self::Profiles(_) | Self::Templates(_) | Self::Sandboxes(_) => {
                "OpenShell"
            }
            Self::Claim(_) => "Hub's trusted claim",
            Self::Hub(_) => "Hub",
        }
    }
}

/// Kubernetes objects: manifest files, one kustomization and ConfigMaps built from
/// repository files. Objects are named by their subject, `<Kind> <namespace>/<name>`.
#[derive(Debug, Clone, Default)]
pub struct KubernetesObjects {
    pub manifests: Vec<PathBuf>,
    pub kustomize: Option<PathBuf>,
    pub config_maps: Vec<ConfigMap>,
    /// Workloads that read an object only when they start, restarted when it changes.
    pub restart: BTreeMap<String, Vec<String>>,
    /// Workloads that become ready only once Hub serves, such as Lore, which reads Hub's
    /// issuer at startup; apply checks them after the last step.
    pub after_hub: Vec<String>,
}

/// A ConfigMap whose keys are repository files, as `kubectl create configmap
/// --from-file=<key>=<file>` makes it.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigMap {
    pub namespace: String,
    pub name: String,
    pub files: BTreeMap<String, PathBuf>,
}

/// A Helm release. The repository's values files come first, then the private ones
/// `private.toml` names for this release.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HelmRelease {
    pub release: String,
    pub namespace: String,
    /// The chart: its name in `repository`, or the chart directory of a source.
    pub chart: String,
    /// The chart repository's URL; a chart from a source has none.
    pub repository: Option<String>,
    /// The chart version; a chart from a source directory has none.
    pub version: Option<String>,
    #[serde(default)]
    pub values: Vec<PathBuf>,
}

/// A provider profile file. A profile without a workspace is global.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub file: PathBuf,
    pub workspace: Option<String>,
}

/// A sandbox template. It names an image from the lock, or none.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Template {
    pub workspace: String,
    pub name: String,
    pub image: Option<String>,
    pub cpu: Option<String>,
    pub memory: Option<String>,
    pub driver_config: Option<PathBuf>,
    #[serde(default)]
    pub environment: BTreeMap<String, String>,
}

/// A long-running platform sandbox.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sandbox {
    pub workspace: String,
    pub name: String,
    /// The image's name in the lock.
    pub image: String,
    pub policy: PathBuf,
    #[serde(default)]
    pub providers: Vec<String>,
    pub driver_config: Option<PathBuf>,
    pub restart_policy: RestartPolicy,
    pub cpu: Option<String>,
    pub memory: Option<String>,
    pub gpu: Option<u32>,
    #[serde(default)]
    pub environment: BTreeMap<String, String>,
    /// Environment variables whose values are the installation's private values, from
    /// `private.toml`.
    #[serde(default)]
    pub private_environment: BTreeSet<String>,
    pub command: Vec<String>,
    #[serde(default, rename = "service")]
    pub services: Vec<Service>,
    /// The server slug it registers with Hub as, under its `VEOVEO_DEPLOYMENT_ID`.
    pub registers: Option<String>,
}

impl Sandbox {
    /// The deployment ID it registers under, from its environment or private values.
    pub const DEPLOYMENT_ID: &str = "VEOVEO_DEPLOYMENT_ID";
}

impl Sandbox {
    pub fn key(&self) -> String {
        format!("{}/{}", self.workspace, self.name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RestartPolicy {
    Always,
    OnFailure,
    Never,
}

impl RestartPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Always => "always",
            Self::OnFailure => "on-failure",
            Self::Never => "never",
        }
    }
}

/// A sandbox port exposed through OpenShell's native service exposure.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Service {
    pub port: u32,
    pub name: String,
    pub authorization: String,
}

/// Hub's control plane.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hub {
    /// The Hub profile whose administrative route the operator signs in to.
    pub profile: String,
    /// The seed documents, merged by their typed keys.
    pub seed: Vec<PathBuf>,
    /// The file published as `computers.handbook`.
    pub handbook: PathBuf,
    /// The claim step whose files the seed names as `@claim:<name>@`.
    pub claim: String,
    /// Hub's sandbox, restarted when a member Hub reads only at its start changes.
    pub workspace: String,
    pub sandbox: String,
}

/// The files of Hub's trusted claim that come from this repository. Each is named by
/// its content, `<name>-<the first 12 hexadecimal digits of its SHA-256>.yaml`, so a
/// changed file is a new file and the control plane names the one it uses. They are
/// read in Hub's sandbox and written by the claim's seed Job, from a temporary Secret.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HubClaim {
    pub workspace: String,
    pub sandbox: String,
    /// Where Hub's sandbox mounts the claim.
    pub directory: String,
    /// The manifest holding the claim's seed Job, which copies every key of its Secret.
    pub manifest: PathBuf,
    pub job: String,
    pub secret: String,
    #[serde(rename = "file")]
    pub files: Vec<ClaimFile>,
    /// The claim's private files, prepared in the private directory's `[identity]` directory.
    #[serde(default)]
    pub private: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimFile {
    pub name: String,
    pub source: PathBuf,
}

impl Declaration {
    pub fn read(repository: &Path) -> Result<Self> {
        let path = repository.join(DECLARATION_FILE);
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("cannot read {}", path.display()))?;
        Self::parse(&text).with_context(|| format!("{} is not a valid declaration", path.display()))
    }

    pub fn parse(text: &str) -> Result<Self> {
        let file: DeclarationFile = toml::from_str(text)?;
        let mut names = BTreeSet::new();
        let mut steps = Vec::with_capacity(file.steps.len());
        for step in file.steps {
            ensure!(!step.name.is_empty(), "a step has no name");
            ensure!(
                names.insert(step.name.clone()),
                "step {} is declared twice",
                step.name
            );
            let name = step.name.clone();
            let kind = step
                .kind()
                .with_context(|| format!("step {name} is not valid"))?;
            steps.push(Step { name, kind });
        }
        let declaration = Self { steps };
        declaration.check_unique_targets()?;
        declaration.check_hub_claim()?;
        Ok(declaration)
    }

    /// No two steps declare the same sandbox, template or profile.
    fn check_unique_targets(&self) -> Result<()> {
        let mut seen = BTreeSet::new();
        for step in &self.steps {
            let targets: Vec<String> = match &step.kind {
                StepKind::Sandboxes(sandboxes) => sandboxes
                    .iter()
                    .map(|sandbox| format!("sandbox {}", sandbox.key()))
                    .collect(),
                StepKind::Templates(templates) => templates
                    .iter()
                    .map(|template| format!("template {}/{}", template.workspace, template.name))
                    .collect(),
                StepKind::Profiles(profiles) => profiles
                    .iter()
                    .map(|profile| format!("profile file {}", profile.file.display()))
                    .collect(),
                _ => Vec::new(),
            };
            for target in targets {
                ensure!(seen.insert(target.clone()), "{target} is declared twice");
            }
        }
        Ok(())
    }

    /// A Hub step names a claim step that comes before it.
    fn check_hub_claim(&self) -> Result<()> {
        for (position, step) in self.steps.iter().enumerate() {
            if let StepKind::Hub(hub) = &step.kind {
                let claim = self.steps[..position].iter().any(|earlier| {
                    earlier.name == hub.claim && matches!(earlier.kind, StepKind::Claim(_))
                });
                ensure!(
                    claim,
                    "Hub step {} names no earlier claim step {}",
                    step.name,
                    hub.claim
                );
            }
        }
        Ok(())
    }

    /// The claim step named `name`.
    pub fn claim(&self, name: &str) -> Option<&HubClaim> {
        self.steps.iter().find_map(|step| match &step.kind {
            StepKind::Claim(claim) if step.name == name => Some(claim),
            _ => None,
        })
    }

    /// Every repository file or directory the steps name.
    pub fn files(&self) -> Vec<&Path> {
        let mut files: Vec<&Path> = Vec::new();
        for step in &self.steps {
            match &step.kind {
                StepKind::Kubernetes(objects) => {
                    files.extend(objects.manifests.iter().map(PathBuf::as_path));
                    files.extend(objects.kustomize.as_deref());
                    for config_map in &objects.config_maps {
                        files.extend(config_map.files.values().map(PathBuf::as_path));
                    }
                }
                StepKind::Helm(release) => {
                    files.extend(release.values.iter().map(PathBuf::as_path))
                }
                StepKind::Workspaces(_) => {}
                StepKind::Profiles(profiles) => {
                    files.extend(profiles.iter().map(|profile| profile.file.as_path()));
                }
                StepKind::Templates(templates) => {
                    files.extend(
                        templates
                            .iter()
                            .filter_map(|template| template.driver_config.as_deref()),
                    );
                }
                StepKind::Sandboxes(sandboxes) => {
                    for sandbox in sandboxes {
                        files.push(&sandbox.policy);
                        files.extend(sandbox.driver_config.as_deref());
                    }
                }
                StepKind::Claim(claim) => {
                    files.push(&claim.manifest);
                    files.extend(claim.files.iter().map(|file| file.source.as_path()));
                }
                StepKind::Hub(hub) => {
                    files.extend(hub.seed.iter().map(PathBuf::as_path));
                    files.push(&hub.handbook);
                }
            }
        }
        files
    }

    pub fn step(&self, name: &str) -> Option<&Step> {
        self.steps.iter().find(|step| step.name == name)
    }

    pub fn sandboxes(&self) -> impl Iterator<Item = &Sandbox> {
        self.steps.iter().flat_map(|step| match &step.kind {
            StepKind::Sandboxes(sandboxes) => sandboxes.as_slice(),
            _ => &[],
        })
    }

    pub fn helm_releases(&self) -> impl Iterator<Item = &HelmRelease> {
        self.steps.iter().filter_map(|step| match &step.kind {
            StepKind::Helm(release) => Some(release),
            _ => None,
        })
    }
}

impl StepFile {
    fn kind(self) -> Result<StepKind> {
        let kubernetes =
            !self.manifests.is_empty() || self.kustomize.is_some() || !self.config_maps.is_empty();
        let kinds = [
            kubernetes,
            self.helm.is_some(),
            !self.workspaces.is_empty(),
            !self.profiles.is_empty(),
            !self.templates.is_empty(),
            !self.sandboxes.is_empty(),
            self.claim.is_some(),
            self.hub.is_some(),
        ];
        match kinds.iter().filter(|declared| **declared).count() {
            0 => bail!("it declares nothing"),
            1 => {}
            _ => bail!("it declares more than one kind of thing; give each its own step"),
        }
        if kubernetes {
            return Ok(StepKind::Kubernetes(KubernetesObjects {
                manifests: self.manifests,
                kustomize: self.kustomize,
                config_maps: self.config_maps,
                restart: self.restart,
                after_hub: self.after_hub,
            }));
        }
        if let Some(helm) = self.helm {
            return Ok(StepKind::Helm(helm));
        }
        if !self.workspaces.is_empty() {
            return Ok(StepKind::Workspaces(self.workspaces));
        }
        if !self.profiles.is_empty() {
            return Ok(StepKind::Profiles(self.profiles));
        }
        if !self.templates.is_empty() {
            return Ok(StepKind::Templates(self.templates));
        }
        if !self.sandboxes.is_empty() {
            for sandbox in &self.sandboxes {
                ensure!(
                    !sandbox.command.is_empty(),
                    "sandbox {} has no command",
                    sandbox.key()
                );
                if let Some(key) = sandbox
                    .private_environment
                    .iter()
                    .find(|key| sandbox.environment.contains_key(*key))
                {
                    bail!(
                        "sandbox {} declares {key} both in environment and private_environment",
                        sandbox.key()
                    );
                }
            }
            return Ok(StepKind::Sandboxes(self.sandboxes));
        }
        if let Some(claim) = self.claim {
            let mut names = BTreeSet::new();
            for file in &claim.files {
                ensure!(
                    names.insert(&file.name),
                    "claim file {} is declared twice",
                    file.name
                );
            }
            return Ok(StepKind::Claim(claim));
        }
        let hub = self.hub.expect("one kind is declared");
        ensure!(!hub.seed.is_empty(), "the Hub step names no seed document");
        Ok(StepKind::Hub(hub))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_keep_their_order_and_kind() {
        let declaration = Declaration::parse(
            r#"
            [[step]]
            name = "namespaces"
            manifests = ["kubernetes/namespaces.yaml"]

            [[step]]
            name = "cilium"
            [step.helm]
            release = "cilium"
            namespace = "kube-system"
            chart = "cilium/cilium"
            version = "1.20.3"
            values = ["kubernetes/cilium.values.yaml"]

            [[step]]
            name = "workspaces"
            workspaces = ["vv-control"]
            "#,
        )
        .unwrap();
        let names: Vec<_> = declaration
            .steps
            .iter()
            .map(|step| step.name.as_str())
            .collect();
        assert_eq!(names, ["namespaces", "cilium", "workspaces"]);
        assert!(matches!(declaration.steps[0].kind, StepKind::Kubernetes(_)));
        assert!(
            matches!(&declaration.steps[1].kind, StepKind::Helm(release) if release.version.as_deref() == Some("1.20.3"))
        );
        assert_eq!(declaration.steps[2].kind.owner(), "OpenShell");
    }

    #[test]
    fn a_step_holds_one_kind_of_thing() {
        let error = Declaration::parse(
            r#"
            [[step]]
            name = "mixed"
            manifests = ["kubernetes/namespaces.yaml"]
            workspaces = ["vv-control"]
            "#,
        )
        .unwrap_err();
        assert!(
            format!("{error:#}").contains("more than one kind"),
            "{error:#}"
        );
    }

    #[test]
    fn a_step_name_and_a_sandbox_are_declared_once() {
        let twice = Declaration::parse(
            r#"
            [[step]]
            name = "a"
            workspaces = ["x"]
            [[step]]
            name = "a"
            workspaces = ["y"]
            "#,
        )
        .unwrap_err();
        assert!(format!("{twice:#}").contains("declared twice"));
        let sandbox = r#"
            workspace = "vv-services"
            name = "console"
            image = "console"
            policy = "p.yaml"
            restart_policy = "always"
            command = ["/usr/local/bin/veoveo-mcp-console"]
        "#;
        let twice = Declaration::parse(&format!(
            "[[step]]\nname = \"a\"\n[[step.sandbox]]\n{sandbox}\n[[step]]\nname = \"b\"\n[[step.sandbox]]\n{sandbox}\n"
        ))
        .unwrap_err();
        assert!(format!("{twice:#}").contains("sandbox vv-services/console is declared twice"));
    }

    #[test]
    fn unknown_members_are_refused() {
        let error = Declaration::parse(
            r#"
            [[step]]
            name = "a"
            workspace = ["typo"]
            "#,
        )
        .unwrap_err();
        assert!(format!("{error:#}").contains("unknown field"), "{error:#}");
    }
}
