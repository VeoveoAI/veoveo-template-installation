//! The operator's private directory: installation values and secrets that never enter
//! Git. Its `private.toml` names the operator's sign-ins and the private inputs of the
//! repository's steps; paths in it are relative to the directory.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use serde::Deserialize;

pub const PRIVATE_FILE: &str = "private.toml";

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Private {
    #[serde(default)]
    pub kubernetes: KubernetesAccess,
    pub openshell: Option<OpenShellAccess>,
    #[serde(default)]
    pub hub: HubAccess,
    /// Private values files of each Helm release, layered over the repository's.
    #[serde(default)]
    pub helm: BTreeMap<String, HelmValues>,
    /// Private environment values of each sandbox, keyed `<workspace>/<name>`.
    #[serde(default)]
    pub sandbox: BTreeMap<String, SandboxValues>,
    /// The identity provider's registration, from which a later phase prepares Hub's
    /// trusted files. `plan` does not read it.
    pub identity: Option<toml::Table>,
    #[serde(skip)]
    directory: PathBuf,
}

/// Which Kubernetes context to use; the default is the kubeconfig's current one.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KubernetesAccess {
    pub context: Option<String>,
}

/// The operator's OpenShell CLI sign-in: the gateway's name and the configuration
/// home (the CLI's `XDG_CONFIG_HOME`) that holds it.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenShellAccess {
    pub gateway: String,
    pub config_home: Option<PathBuf>,
}

/// Where the operator's Hub sign-in is stored; the default is the Hub SDK CLI's grant
/// directory, so one sign-in serves both.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HubAccess {
    pub grants: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HelmValues {
    #[serde(default)]
    pub values: Vec<PathBuf>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SandboxValues {
    #[serde(default)]
    pub environment: BTreeMap<String, String>,
}

impl Private {
    /// Reads `private.toml` from `directory`, which only its owner may read.
    pub fn read(directory: &Path) -> Result<Self> {
        check_owner_only(directory)?;
        let path = directory.join(PRIVATE_FILE);
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("cannot read {}", path.display()))?;
        let mut private =
            Self::parse(&text).with_context(|| format!("{} is not valid", path.display()))?;
        private.directory = directory.to_path_buf();
        Ok(private)
    }

    pub fn parse(text: &str) -> Result<Self> {
        Ok(toml::from_str(text)?)
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// A private file, by its path relative to the directory.
    pub fn path(&self, relative: &Path) -> PathBuf {
        self.directory.join(relative)
    }

    pub fn helm_values(&self, release: &str) -> Vec<PathBuf> {
        self.helm
            .get(release)
            .map(|values| values.values.iter().map(|file| self.path(file)).collect())
            .unwrap_or_default()
    }

    pub fn sandbox_environment(&self, key: &str) -> Option<&BTreeMap<String, String>> {
        self.sandbox.get(key).map(|values| &values.environment)
    }
}

#[cfg(unix)]
fn check_owner_only(directory: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    let metadata = std::fs::metadata(directory)
        .with_context(|| format!("cannot read the private directory {}", directory.display()))?;
    ensure!(
        metadata.is_dir(),
        "{} is not a directory",
        directory.display()
    );
    let mode = metadata.permissions().mode() & 0o777;
    ensure!(
        mode & 0o077 == 0,
        "the private directory {} has mode {mode:03o}; only its owner may use it (0700)",
        directory.display()
    );
    Ok(())
}

#[cfg(not(unix))]
fn check_owner_only(directory: &Path) -> Result<()> {
    ensure!(
        directory.is_dir(),
        "{} is not a directory",
        directory.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_values_parse_and_resolve_relative_paths() {
        let mut private = Private::parse(
            r#"
            [openshell]
            gateway = "k3s-user"

            [helm.openshell]
            values = ["rendered/openshell-identity.yaml"]

            [sandbox."vv-services/console".environment]
            VEOVEO_DEPLOYMENT_ID = "0199e3a0-0000-7000-8000-000000000000"
            "#,
        )
        .unwrap();
        private.directory = PathBuf::from("/private");
        assert_eq!(private.openshell.as_ref().unwrap().gateway, "k3s-user");
        assert_eq!(
            private.helm_values("openshell"),
            [PathBuf::from("/private/rendered/openshell-identity.yaml")]
        );
        assert!(private.helm_values("cilium").is_empty());
        assert!(
            private
                .sandbox_environment("vv-services/console")
                .unwrap()
                .contains_key("VEOVEO_DEPLOYMENT_ID")
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_directory_others_can_read_is_refused() {
        use std::os::unix::fs::PermissionsExt as _;
        let directory =
            std::env::temp_dir().join(format!("veoveo-installer-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o755)).unwrap();
        let error = Private::read(&directory).unwrap_err();
        assert!(error.to_string().contains("0700"), "{error:#}");
        std::fs::remove_dir_all(&directory).unwrap();
    }
}
