//! Helm releases, read with the `helm` CLI: the release's chart and status from
//! `helm list`, and the values it was installed with from `helm get values`, against
//! the repository's values files merged with the private ones as Helm merges them.

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::Value;

use crate::{
    action::{Action, ValuesFile},
    declaration::HelmRelease,
    difference::{Difference, Path as ValuePath, compare, merge_values},
    inputs::Inputs,
    plan::{Item, State},
};

/// One row of `helm list -o json`.
#[derive(Debug, Deserialize)]
pub struct Listed {
    pub name: String,
    pub chart: String,
    pub status: String,
    #[serde(default)]
    pub revision: String,
}

pub async fn plan(inputs: &Inputs, release: &HelmRelease) -> Vec<Item> {
    let subject = format!("release {}/{}", release.namespace, release.release);
    let (layers, files) = match layers(inputs, release) {
        Ok(layers) => layers,
        Err(error) => return vec![Item::new(subject, State::NotCompared(format!("{error:#}")))],
    };
    let declared = match declared_values(&layers) {
        Ok(values) => values,
        Err(error) => return vec![Item::new(subject, State::NotCompared(format!("{error:#}")))],
    };
    match unfilled(inputs, release).and_then(|values| digest_checks(&values, &inputs.lock)) {
        Ok(()) => {}
        Err(error) => return vec![Item::new(subject, State::Refused(format!("{error:#}")))],
    }
    let upgrade = Action::UpgradeRelease {
        release: release.clone(),
        values: files,
    };
    let listed = match list(release).await {
        Ok(Some(listed)) => listed,
        Ok(None) => {
            return vec![Item::new(subject, State::Absent { note: None }).with_action(upgrade)];
        }
        Err(error) => return vec![Item::unread(subject, format!("{error:#}"))],
    };
    let active = match values(release).await {
        Ok(values) => values,
        Err(error) => return vec![Item::unread(subject, format!("{error:#}"))],
    };
    let mut differences = release_differences(release, &listed);
    differences.extend(compare(&declared, &active).into_iter().map(|difference| {
        Difference::new(
            difference.path.under("values"),
            difference.declared,
            difference.active,
        )
    }));
    let note = Some(format!("revision {}", listed.revision));
    vec![Item::compared(subject, differences, note).with_action(upgrade)]
}

/// The chart and status differences of a listed release.
pub fn release_differences(release: &HelmRelease, listed: &Listed) -> Vec<Difference> {
    let mut differences = Vec::new();
    if let Some(version) = &release.version {
        let chart_name = release.chart.rsplit('/').next().unwrap_or(&release.chart);
        let declared = format!("{chart_name}-{version}");
        if listed.chart != declared {
            differences.push(Difference::new(
                ValuePath::root().member("chart"),
                Some(Value::String(declared)),
                Some(Value::String(listed.chart.clone())),
            ));
        }
    }
    if listed.status != "deployed" {
        differences.push(Difference::new(
            ValuePath::root().member("status"),
            Some(Value::String("deployed".to_owned())),
            Some(Value::String(listed.status.clone())),
        ));
    }
    differences
}

/// Values files as (name, text), in the order Helm merges them.
type Layers = Vec<(String, String)>;

/// The release's values files as (name, text), and as files for `helm upgrade`: the
/// repository's, filled from the lock, then the private ones.
fn layers(inputs: &Inputs, release: &HelmRelease) -> Result<(Layers, Vec<ValuesFile>)> {
    let mut layers = Vec::new();
    let mut files = Vec::new();
    for file in &release.values {
        let text = inputs.filled(file)?;
        layers.push((file.display().to_string(), text.clone()));
        files.push(ValuesFile::Repository {
            name: file.display().to_string(),
            text,
        });
    }
    for file in inputs.private.helm_values(&release.release) {
        let text = std::fs::read_to_string(&file)
            .with_context(|| format!("cannot read the private values file {}", file.display()))?;
        layers.push((file.display().to_string(), text));
        files.push(ValuesFile::Private { path: file });
    }
    Ok((layers, files))
}

/// The release's values merged before the lock fills them.
fn unfilled(inputs: &Inputs, release: &HelmRelease) -> Result<Value> {
    let mut layers = Vec::new();
    for file in &release.values {
        layers.push((file.display().to_string(), inputs.text(file)?));
    }
    for file in inputs.private.helm_values(&release.release) {
        let text = std::fs::read_to_string(&file)
            .with_context(|| format!("cannot read the private values file {}", file.display()))?;
        layers.push((file.display().to_string(), text));
    }
    declared_values(&layers)
}

/// Each image the values name by `@digest:<name>@` beside its `repository`, and the
/// image's own `registry` or `global.image.registry`, is the lock entry's repository as
/// a chart composes it, `<registry>/<repository>`; the digest alone cannot say so.
pub fn digest_checks(
    values: &Value,
    lock: &veoveo_installation_contract::ComponentLock,
) -> Result<()> {
    let global = values
        .pointer("/global/image/registry")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let mut problems = Vec::new();
    collect_digests(values, "", &mut |path, image| {
        let Some(name) = image["digest"]
            .as_str()
            .and_then(|digest| digest.strip_prefix("@digest:"))
            .and_then(|rest| rest.strip_suffix('@'))
        else {
            return;
        };
        let locked = match crate::inputs::image(lock, name) {
            Ok(reference) => reference
                .split_once('@')
                .map(|(repository, _)| repository.to_owned())
                .unwrap_or(reference),
            Err(error) => return problems.push(format!("{path}: {error}")),
        };
        let Some(repository) = image["repository"]
            .as_str()
            .filter(|repository| !repository.is_empty())
        else {
            return problems.push(format!("{path} names no repository beside @digest:{name}@"));
        };
        let registry = image["registry"]
            .as_str()
            .filter(|registry| !registry.is_empty())
            .unwrap_or(global);
        let written = if registry.is_empty() {
            repository.to_owned()
        } else {
            format!("{registry}/{repository}")
        };
        if written != locked {
            problems.push(format!(
                "{path} names {written}, but the lock's {name} is {locked}"
            ));
        }
    });
    if problems.is_empty() {
        Ok(())
    } else {
        bail!("{}", problems.join("; "))
    }
}

fn collect_digests(value: &Value, path: &str, found: &mut impl FnMut(&str, &Value)) {
    if let Value::Object(members) = value {
        if members
            .get("digest")
            .and_then(Value::as_str)
            .is_some_and(|digest| digest.starts_with("@digest:"))
        {
            found(path, value);
        }
        for (name, member) in members {
            let path = if path.is_empty() {
                name.clone()
            } else {
                format!("{path}.{name}")
            };
            collect_digests(member, &path, found);
        }
    }
}

/// The chart argument of `helm upgrade`: a repository chart's name, or a chart directory
/// in the source checkout private.toml names.
pub fn chart_reference(inputs: &Inputs, release: &HelmRelease) -> Result<String> {
    if release.repository.is_some() {
        return Ok(release.chart.clone());
    }
    let source = inputs
        .private
        .helm
        .get(&release.release)
        .and_then(|values| values.source.clone())
        .with_context(|| {
            format!(
                "private.toml names no source checkout for the chart of release {}",
                release.release
            )
        })?;
    Ok(source.join(&release.chart).display().to_string())
}

/// The arguments of `helm upgrade --install` for the release, its chart and values files.
pub fn upgrade_arguments(release: &HelmRelease, chart: &str, files: &[String]) -> Vec<String> {
    let mut arguments = vec![
        "upgrade".to_owned(),
        "--install".to_owned(),
        release.release.clone(),
        chart.to_owned(),
        "--namespace".to_owned(),
        release.namespace.clone(),
    ];
    if let Some(repository) = &release.repository {
        arguments.extend(["--repo".to_owned(), repository.clone()]);
    }
    if let Some(version) = &release.version {
        arguments.extend(["--version".to_owned(), version.clone()]);
    }
    for file in files {
        arguments.extend(["--values".to_owned(), file.clone()]);
    }
    arguments.extend([
        "--wait".to_owned(),
        "--timeout".to_owned(),
        "15m".to_owned(),
        "--history-max".to_owned(),
        "20".to_owned(),
    ]);
    arguments
}

/// Upgrades or installs the release with exactly the declared values and waits for it.
/// The repository's filled values go to a private temporary directory, removed after.
pub async fn upgrade(inputs: &Inputs, release: &HelmRelease, values: &[ValuesFile]) -> Result<()> {
    let chart = chart_reference(inputs, release)?;
    let directory = std::env::temp_dir().join(format!(
        "veoveo-installer-{}-{}",
        std::process::id(),
        release.release
    ));
    create_private_directory(&directory)?;
    let mut files = Vec::new();
    for (position, file) in values.iter().enumerate() {
        match file {
            ValuesFile::Repository { text, .. } => {
                let path = directory.join(format!("{position}.yaml"));
                std::fs::write(&path, text)
                    .with_context(|| format!("cannot write {}", path.display()))?;
                files.push(path.display().to_string());
            }
            ValuesFile::Private { path } => files.push(path.display().to_string()),
        }
    }
    let arguments = upgrade_arguments(release, &chart, &files);
    let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
    let result = helm(&arguments).await;
    let _ = std::fs::remove_dir_all(&directory);
    result.map(|_| ())
}

fn create_private_directory(directory: &std::path::Path) -> Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder
        .create(directory)
        .with_context(|| format!("cannot create {}", directory.display()))
}

/// The values files merged in order.
pub fn declared_values(layers: &[(String, String)]) -> Result<Value> {
    let mut merged = Value::Object(serde_json::Map::new());
    for (name, text) in layers {
        merge_values(&mut merged, read_values(name, text)?);
    }
    Ok(merged)
}

fn read_values(name: &str, text: &str) -> Result<Value> {
    let value = crate::yaml::value(text).with_context(|| format!("{name} is not valid YAML"))?;
    Ok(if value.is_null() {
        Value::Object(serde_json::Map::new())
    } else {
        value
    })
}

async fn helm(arguments: &[&str]) -> Result<String> {
    let output = tokio::process::Command::new("helm")
        .args(arguments)
        .output()
        .await
        .context("cannot run helm")?;
    if !output.status.success() {
        bail!(
            "helm {} failed: {}",
            arguments.first().copied().unwrap_or_default(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8(output.stdout)?)
}

async fn list(release: &HelmRelease) -> Result<Option<Listed>> {
    let filter = format!("^{}$", release.release);
    let text = helm(&[
        "list",
        "--namespace",
        &release.namespace,
        "--filter",
        &filter,
        "--all",
        "--output",
        "json",
    ])
    .await?;
    let listed: Vec<Listed> = serde_json::from_str(&text).context("helm list answered no JSON")?;
    Ok(listed
        .into_iter()
        .find(|listed| listed.name == release.release))
}

async fn values(release: &HelmRelease) -> Result<Value> {
    let text = helm(&[
        "get",
        "values",
        &release.release,
        "--namespace",
        &release.namespace,
        "--output",
        "json",
    ])
    .await?;
    let value: Value = serde_json::from_str(&text).context("helm get values answered no JSON")?;
    Ok(if value.is_null() {
        Value::Object(serde_json::Map::new())
    } else {
        value
    })
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::*;

    fn release(version: Option<&str>) -> HelmRelease {
        HelmRelease {
            release: "cilium".into(),
            namespace: "kube-system".into(),
            chart: "cilium".into(),
            repository: Some("https://helm.cilium.io/".into()),
            version: version.map(str::to_owned),
            values: vec![],
        }
    }

    #[test]
    fn the_chart_version_and_status_are_compared() {
        let listed = Listed {
            name: "cilium".into(),
            chart: "cilium-1.20.2".into(),
            status: "failed".into(),
            revision: "3".into(),
        };
        let differences = release_differences(&release(Some("1.20.3")), &listed);
        assert_eq!(
            differences
                .iter()
                .map(|difference| difference.path.to_string())
                .collect::<Vec<_>>(),
            ["chart", "status"]
        );
        // A chart from a source directory has no version to compare.
        let listed = Listed {
            status: "deployed".into(),
            ..listed
        };
        assert!(release_differences(&release(None), &listed).is_empty());
    }

    #[test]
    fn private_values_layer_over_the_repository_values() {
        let layers = [
            (
                "kubernetes/openshell.values.yaml".to_owned(),
                "gateway:\n  image:\n    digest: \"\"\n  replicas: 1\nlist: [a, b]\n".to_owned(),
            ),
            (
                "private.yaml".to_owned(),
                "gateway:\n  image:\n    digest: sha256:1\nlist: [c]\n".to_owned(),
            ),
        ];
        assert_eq!(
            declared_values(&layers).unwrap(),
            json!({"gateway": {"image": {"digest": "sha256:1"}, "replicas": 1}, "list": ["c"]})
        );
    }

    #[test]
    fn value_paths_are_shown_under_values() {
        let differences = compare(
            &json!({"a": {"items": [{"name": "x", "b": 1}]}}),
            &json!({"a": {"items": [{"name": "x", "b": 2}]}}),
        );
        assert_eq!(
            differences[0].path.under("values").to_string(),
            "values.a.items[name=x].b"
        );
    }
}

#[cfg(test)]
mod upgrade_tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn an_upgrade_sets_exactly_the_declared_values_and_waits() {
        let release = HelmRelease {
            release: "seaweedfs".into(),
            namespace: "veoveo-system".into(),
            chart: "seaweedfs".into(),
            repository: Some("https://seaweedfs.github.io/seaweedfs/helm".into()),
            version: Some("4.48.0".into()),
            values: vec![],
        };
        assert_eq!(
            upgrade_arguments(
                &release,
                "seaweedfs",
                &["/tmp/v/0.yaml".to_owned(), "/private/s.yaml".to_owned()]
            ),
            [
                "upgrade",
                "--install",
                "seaweedfs",
                "seaweedfs",
                "--namespace",
                "veoveo-system",
                "--repo",
                "https://seaweedfs.github.io/seaweedfs/helm",
                "--version",
                "4.48.0",
                "--values",
                "/tmp/v/0.yaml",
                "--values",
                "/private/s.yaml",
                "--wait",
                "--timeout",
                "15m",
                "--history-max",
                "20",
            ]
        );
        // No --reuse-values: the release's values become exactly the declared ones.
        let source = HelmRelease {
            repository: None,
            version: None,
            ..release
        };
        let arguments = upgrade_arguments(&source, "/src/OpenShell/deploy/helm/openshell", &[]);
        assert!(
            !arguments
                .iter()
                .any(|argument| argument == "--repo" || argument == "--reuse-values")
        );
    }
}

#[cfg(test)]
mod digest_tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_digest_names_the_lock_entrys_repository_as_the_chart_composes_it() {
        let lock = crate::inputs::tests::lock();
        let values = json!({
            "global": {"image": {"registry": "registry.example.com:5000/veoveo"}},
            "gateway": {"image": {"repository": "lore", "digest": "@digest:lore@"}},
            "supervisor": {"image": {"registry": "registry.example.com:5000", "repository": "veoveo/hub-sdk-cli", "digest": "@digest:hub-sdk-cli@"}},
        });
        assert!(digest_checks(&values, &lock).is_ok());
        let wrong = json!({
            "global": {"image": {"registry": "registry.example.com:5000/veoveo"}},
            "gateway": {"image": {"repository": "openshell/gateway", "digest": "@digest:lore@"}},
        });
        let error = digest_checks(&wrong, &lock).unwrap_err().to_string();
        assert_eq!(
            error,
            "gateway.image names registry.example.com:5000/veoveo/openshell/gateway, but the lock's lore is registry.example.com:5000/veoveo/lore"
        );
        let missing = json!({"gateway": {"image": {"digest": "@digest:lore@"}}});
        assert!(
            digest_checks(&missing, &lock)
                .unwrap_err()
                .to_string()
                .contains("names no repository")
        );
    }
}
