//! Kubernetes objects, compared through the API: the object as a server-side apply
//! would leave it, from a dry run, against the object as it is. Fields an earlier
//! `kubectl apply` set that this repository no longer declares are listed too, from the
//! object's last-applied configuration, since a server-side apply would leave them.

use std::collections::{BTreeMap, HashMap};

use anyhow::{Context, Result, anyhow, bail};
use kube::{
    Client,
    api::{Api, DynamicObject, Patch, PatchParams},
    config::{Config, KubeConfigOptions},
    core::GroupVersionKind,
    discovery::{ApiCapabilities, ApiResource, Scope, pinned_kind},
};
use serde_json::{Value, json};

use crate::{
    declaration::{ConfigMap, KubernetesObjects},
    difference::{Difference, compare, remove},
    inputs::Inputs,
    plan::{Item, State},
};

pub const FIELD_MANAGER: &str = "veoveo-installer";
const LAST_APPLIED: &str = "kubectl.kubernetes.io/last-applied-configuration";
/// Annotations the server writes on every change of a workload's template.
const SERVER_ANNOTATIONS: [&str; 2] = [
    "deployment.kubernetes.io/revision",
    "deprecated.daemonset.template.generation",
];

/// One declared object, or a file that could not be read into objects.
#[derive(Debug)]
pub enum Declared {
    Object(Value),
    Unusable { subject: String, reason: String },
}

/// The objects a step declares, in file order.
pub fn declared(inputs: &Inputs, objects: &KubernetesObjects) -> Vec<Declared> {
    let mut declared = Vec::new();
    for config_map in &objects.config_maps {
        match config_map_object(inputs, config_map) {
            Ok(object) => declared.push(Declared::Object(object)),
            Err(error) => declared.push(Declared::Unusable {
                subject: format!("ConfigMap {}/{}", config_map.namespace, config_map.name),
                reason: format!("{error:#}"),
            }),
        }
    }
    for manifest in &objects.manifests {
        let parsed = inputs
            .filled(manifest)
            .and_then(|text| parse_manifests(&text));
        match parsed {
            Ok(objects) => declared.extend(objects.into_iter().map(Declared::Object)),
            Err(error) => declared.push(Declared::Unusable {
                subject: manifest.display().to_string(),
                reason: format!("{error:#}"),
            }),
        }
    }
    if let Some(directory) = &objects.kustomize {
        match kustomize(inputs, directory).and_then(|text| parse_manifests(&text)) {
            Ok(objects) => declared.extend(objects.into_iter().map(Declared::Object)),
            Err(error) => declared.push(Declared::Unusable {
                subject: directory.display().to_string(),
                reason: format!("{error:#}"),
            }),
        }
    }
    declared
}

/// The objects of a YAML stream; empty documents are skipped.
pub fn parse_manifests(text: &str) -> Result<Vec<Value>> {
    let mut objects = Vec::new();
    for value in crate::yaml::documents(text).context("the YAML is not valid")? {
        if value.is_null() {
            continue;
        }
        if value.get("kind").and_then(Value::as_str).is_none() {
            bail!("a document has no kind");
        }
        objects.push(value);
    }
    Ok(objects)
}

fn config_map_object(inputs: &Inputs, config_map: &ConfigMap) -> Result<Value> {
    let mut data = serde_json::Map::new();
    for (key, file) in &config_map.files {
        data.insert(key.clone(), Value::String(inputs.text(file)?));
    }
    Ok(json!({
        "apiVersion": "v1",
        "kind": "ConfigMap",
        "metadata": {"name": config_map.name, "namespace": config_map.namespace},
        "data": data,
    }))
}

/// Renders a kustomization with kubectl's built-in kustomize.
fn kustomize(inputs: &Inputs, directory: &std::path::Path) -> Result<String> {
    let output = std::process::Command::new("kubectl")
        .arg("kustomize")
        .arg(inputs.path(directory))
        .output()
        .context("cannot run kubectl kustomize")?;
    if !output.status.success() {
        bail!(
            "kubectl kustomize failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8(output.stdout)?)
}

pub fn subject(object: &Value) -> String {
    let kind = object["kind"].as_str().unwrap_or("object");
    let name = object["metadata"]["name"].as_str().unwrap_or("(unnamed)");
    match object["metadata"]["namespace"].as_str() {
        Some(namespace) => format!("{kind} {namespace}/{name}"),
        None => format!("{kind} {name}"),
    }
}

/// A Kubernetes API client from the kubeconfig, at its current or a named context.
pub async fn connect(context: Option<&str>) -> Result<Client> {
    let options = KubeConfigOptions {
        context: context.map(str::to_owned),
        cluster: None,
        user: None,
    };
    let config = Config::from_kubeconfig(&options)
        .await
        .context("cannot read the kubeconfig")?;
    Client::try_from(config).context("cannot build a Kubernetes client")
}

/// Compares declared objects with the cluster.
pub struct Cluster {
    client: Client,
    kinds: HashMap<String, (ApiResource, ApiCapabilities)>,
    namespaces: BTreeMap<String, bool>,
}

impl Cluster {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            kinds: HashMap::new(),
            namespaces: BTreeMap::new(),
        }
    }

    pub async fn plan(&mut self, declared: Vec<Declared>) -> Vec<Item> {
        let mut items = Vec::with_capacity(declared.len());
        for declared in declared {
            items.push(match declared {
                Declared::Object(object) => self.plan_object(object).await,
                Declared::Unusable { subject, reason } => {
                    Item::new(subject, State::NotCompared(reason))
                }
            });
        }
        items
    }

    async fn plan_object(&mut self, object: Value) -> Item {
        let subject = subject(&object);
        match self.state(object).await {
            Ok(state) => Item::new(subject, state),
            Err(error) => Item::unread(subject, format!("{error:#}")),
        }
    }

    async fn state(&mut self, object: Value) -> Result<State> {
        let (resource, capabilities) = self.kind(&object).await?;
        let name = object["metadata"]["name"]
            .as_str()
            .context("the object has no name")?
            .to_owned();
        let namespace = object["metadata"]["namespace"].as_str().map(str::to_owned);
        let api: Api<DynamicObject> = match (&capabilities.scope, &namespace) {
            (Scope::Namespaced, Some(namespace)) => {
                Api::namespaced_with(self.client.clone(), namespace, &resource)
            }
            (Scope::Namespaced, None) => bail!("a namespaced object names no namespace"),
            (Scope::Cluster, _) => Api::all_with(self.client.clone(), &resource),
        };
        if let (Scope::Namespaced, Some(namespace)) = (&capabilities.scope, &namespace)
            && !self.namespace_exists(namespace).await?
        {
            return Ok(State::Absent {
                note: Some(format!("its namespace {namespace} does not exist yet")),
            });
        }
        let live = api.get_opt(&name).await.context("cannot read it")?;
        let kind = object["kind"].as_str().unwrap_or_default();
        if kind == "Job" {
            return Ok(State::NotCompared(match live {
                None => {
                    "a seed Job runs once and is deleted; plan does not read its claim".to_owned()
                }
                Some(_) => {
                    "a seed Job that was not deleted; plan does not read its claim".to_owned()
                }
            }));
        }
        let Some(live) = live else {
            return Ok(State::Absent { note: None });
        };
        let live = serde_json::to_value(&live)?;
        let applied: DynamicObject = serde_json::from_value(object.clone())?;
        let params = PatchParams::apply(FIELD_MANAGER).force().dry_run();
        let result = match api.patch(&name, &params, &Patch::Apply(&applied)).await {
            Ok(result) => serde_json::to_value(&result)?,
            Err(kube::Error::Api(status)) => return Ok(State::Refused(status.message.clone())),
            Err(error) => return Err(anyhow!(error).context("the dry run failed")),
        };
        let mut differences = compare(&normalized(result), &normalized(live.clone()));
        differences.extend(no_longer_declared(&object, &live));
        Ok(if differences.is_empty() {
            State::Same
        } else {
            State::Differs {
                differences,
                note: None,
            }
        })
    }

    async fn kind(&mut self, object: &Value) -> Result<(ApiResource, ApiCapabilities)> {
        let api_version = object["apiVersion"]
            .as_str()
            .context("the object has no apiVersion")?;
        let kind = object["kind"].as_str().context("the object has no kind")?;
        let key = format!("{api_version}/{kind}");
        if let Some(found) = self.kinds.get(&key) {
            return Ok(found.clone());
        }
        let (group, version) = match api_version.split_once('/') {
            Some((group, version)) => (group, version),
            None => ("", api_version),
        };
        let gvk = GroupVersionKind::gvk(group, version, kind);
        let found = pinned_kind(&self.client, &gvk)
            .await
            .with_context(|| format!("the cluster does not serve {key}"))?;
        self.kinds.insert(key, found.clone());
        Ok(found)
    }

    async fn namespace_exists(&mut self, namespace: &str) -> Result<bool> {
        if let Some(exists) = self.namespaces.get(namespace) {
            return Ok(*exists);
        }
        let api: Api<k8s_openapi::api::core::v1::Namespace> = Api::all(self.client.clone());
        let exists = api
            .get_opt(namespace)
            .await
            .with_context(|| format!("cannot read namespace {namespace}"))?
            .is_some();
        self.namespaces.insert(namespace.to_owned(), exists);
        Ok(exists)
    }
}

/// An object without what the server keeps for itself: status, identity, versions,
/// field ownership and the annotations it writes.
pub fn normalized(mut object: Value) -> Value {
    for path in [
        "status",
        "metadata.managedFields",
        "metadata.resourceVersion",
        "metadata.generation",
        "metadata.uid",
        "metadata.creationTimestamp",
        "metadata.selfLink",
    ] {
        remove(&mut object, path);
    }
    if let Some(annotations) = object["metadata"]["annotations"].as_object_mut() {
        for annotation in SERVER_ANNOTATIONS {
            annotations.remove(annotation);
        }
    }
    object
}

/// Fields the last `kubectl apply` set that the declaration no longer has. A server-side
/// apply leaves them, so they are shown as active and not declared.
pub fn no_longer_declared(declared: &Value, live: &Value) -> Vec<Difference> {
    let Some(last) = live["metadata"]["annotations"][LAST_APPLIED]
        .as_str()
        .and_then(|text| serde_json::from_str::<Value>(text).ok())
    else {
        return Vec::new();
    };
    let mut declared = declared.clone();
    remove(&mut declared, "status");
    compare(&declared, &last)
        .into_iter()
        .filter(|difference| difference.declared.is_none())
        .filter(|difference| {
            // The annotation itself and server-assigned identity are not declarations.
            let path = difference.path.to_string();
            !path.starts_with("metadata.annotations") && !path.starts_with("status")
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn a_secret_volume_mode_reads_as_kubernetes_reads_it() {
        let objects = parse_manifests(
            "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: lore\nspec:\n  template:\n    spec:\n      volumes:\n        - name: tls\n          secret:\n            defaultMode: 0440\n",
        )
        .unwrap();
        assert_eq!(
            objects[0]["spec"]["template"]["spec"]["volumes"][0]["secret"]["defaultMode"],
            288
        );
    }

    #[test]
    fn manifests_parse_as_ordered_objects() {
        let objects = parse_manifests(
            "# a comment\n---\napiVersion: v1\nkind: Namespace\nmetadata:\n  name: veoveo-system\n---\n---\napiVersion: v1\nkind: Service\nmetadata:\n  name: lore\n  namespace: veoveo-system\n",
        )
        .unwrap();
        assert_eq!(
            objects.iter().map(subject).collect::<Vec<_>>(),
            ["Namespace veoveo-system", "Service veoveo-system/lore"]
        );
        assert!(parse_manifests("metadata: {}\n").is_err());
    }

    #[test]
    fn server_fields_do_not_count_as_differences() {
        // A recorded dry-run result and the live Deployment it was made from.
        let live = json!({
            "apiVersion": "apps/v1", "kind": "Deployment",
            "metadata": {
                "name": "lore", "namespace": "veoveo-system", "uid": "a", "resourceVersion": "10",
                "generation": 4, "creationTimestamp": "2026-10-22T00:00:00Z",
                "managedFields": [{"manager": "kubectl-client-side-apply"}],
                "annotations": {"deployment.kubernetes.io/revision": "4"}
            },
            "spec": {"template": {"spec": {"containers": [{"name": "lore", "image": "lore@sha256:1"}]}}},
            "status": {"replicas": 1}
        });
        let mut result = live.clone();
        result["metadata"]["managedFields"] = json!([{"manager": FIELD_MANAGER}]);
        result["metadata"]["generation"] = json!(5);
        result["metadata"]["annotations"]["deployment.kubernetes.io/revision"] = json!("5");
        assert!(compare(&normalized(result.clone()), &normalized(live.clone())).is_empty());
        result["spec"]["template"]["spec"]["containers"][0]["image"] = json!("lore@sha256:2");
        let differences = compare(&normalized(result), &normalized(live));
        assert_eq!(differences.len(), 1);
        assert_eq!(
            differences[0].path.to_string(),
            "spec.template.spec.containers[name=lore].image"
        );
    }

    #[test]
    fn fields_dropped_from_the_declaration_since_the_last_apply_are_listed() {
        let declared = json!({
            "apiVersion": "v1", "kind": "ConfigMap",
            "metadata": {"name": "lore-settings", "namespace": "veoveo-system"},
            "data": {"local.toml": "[server]\n"}
        });
        let last = json!({
            "apiVersion": "v1", "kind": "ConfigMap",
            "metadata": {"name": "lore-settings", "namespace": "veoveo-system", "labels": {"old": "x"}},
            "data": {"local.toml": "[server]\n[hooks]\n", "extra.toml": "x"}
        });
        let live = json!({"metadata": {"annotations": {LAST_APPLIED: last.to_string()}}});
        let found: Vec<String> = no_longer_declared(&declared, &live)
            .iter()
            .map(|difference| difference.path.to_string())
            .collect();
        assert_eq!(found, [r#"data["extra.toml"]"#, "metadata.labels"]);
    }
}
