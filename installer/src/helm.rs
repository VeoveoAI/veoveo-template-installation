//! Helm releases, read with the `helm` CLI: the release's chart and status from
//! `helm list`, and the values it was installed with from `helm get values`, against
//! the repository's values files merged with the private ones as Helm merges them.

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::Value;

use crate::{
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
    let declared = match layers(inputs, release).and_then(|layers| declared_values(&layers)) {
        Ok(values) => values,
        Err(error) => return vec![Item::new(subject, State::NotCompared(format!("{error:#}")))],
    };
    let listed = match list(release).await {
        Ok(Some(listed)) => listed,
        Ok(None) => return vec![Item::new(subject, State::Absent { note: None })],
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
    vec![Item::compared(subject, differences, note)]
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

/// The release's values files as (name, text): the repository's, filled from the lock,
/// then the private ones.
fn layers(inputs: &Inputs, release: &HelmRelease) -> Result<Vec<(String, String)>> {
    let mut layers = Vec::new();
    for file in &release.values {
        layers.push((file.display().to_string(), inputs.filled(file)?));
    }
    for file in inputs.private.helm_values(&release.release) {
        let text = std::fs::read_to_string(&file)
            .with_context(|| format!("cannot read the private values file {}", file.display()))?;
        layers.push((file.display().to_string(), text));
    }
    Ok(layers)
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
