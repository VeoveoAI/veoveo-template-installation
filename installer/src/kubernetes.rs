//! Kubernetes objects, compared through the API: the object as a server-side apply
//! would leave it, from a dry run, against the object as it is. Fields an earlier
//! `kubectl apply` set that this repository no longer declares are listed too, from the
//! object's last-applied configuration, since a server-side apply would leave them.

use std::{
    collections::{BTreeMap, HashMap},
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow, bail};
use kube::{
    Client,
    api::{Api, DeleteParams, DynamicObject, ListParams, Patch, PatchParams, PostParams},
    config::{Config, KubeConfigOptions},
    core::GroupVersionKind,
    discovery::{ApiCapabilities, ApiResource, Scope, pinned_kind},
};
use serde_json::{Value, json};

use crate::{
    action::Action,
    declaration::{ConfigMap, HubClaim, KubernetesObjects},
    difference::{Difference, compare, remove},
    inputs::Inputs,
    plan::{Item, State},
};

pub const FIELD_MANAGER: &str = "veoveo-installer";
/// The field manager of a restart's annotation, apart from the declaration's, so a later
/// server-side apply keeps it.
pub const RESTART_MANAGER: &str = "veoveo-installer-restart";
const RESTARTED_AT: &str = "kubectl.kubernetes.io/restartedAt";
/// How long a workload's rollout may take.
pub const ROLLOUT_TIMEOUT: Duration = Duration::from_secs(600);
/// How long a claim's seed Job may take.
const JOB_TIMEOUT: Duration = Duration::from_secs(180);
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

    /// Each declared object against the cluster, with the change apply makes: a
    /// server-side apply, the removal of members an earlier `kubectl apply` set that the
    /// declaration no longer has, and the restart of workloads that read the object at
    /// their start.
    pub async fn plan(
        &mut self,
        declared: Vec<Declared>,
        restart: &BTreeMap<String, Vec<String>>,
    ) -> Vec<Item> {
        let mut items = Vec::with_capacity(declared.len());
        for declared in declared {
            items.push(match declared {
                Declared::Object(object) => {
                    let restart = restart.get(&subject(&object)).cloned().unwrap_or_default();
                    self.plan_object(object, restart).await
                }
                Declared::Unusable { subject, reason } => {
                    Item::new(subject, State::NotCompared(reason))
                }
            });
        }
        items
    }

    async fn plan_object(&mut self, object: Value, restart: Vec<String>) -> Item {
        let subject = subject(&object);
        let (state, leftover) = match self.state(object.clone()).await {
            Ok(state) => state,
            Err(error) => return Item::unread(subject, format!("{error:#}")),
        };
        // Members a server-side apply leaves are removed with merge patches; one inside a
        // list cannot be, so apply has no change for the object.
        let remove: Option<Vec<Value>> = leftover
            .iter()
            .map(|difference| difference.path.removal_patch())
            .collect();
        let item = Item::new(subject, state);
        match remove {
            Some(remove) => item.with_action(Action::ApplyObject {
                object,
                remove,
                restart,
            }),
            None => item,
        }
    }

    /// How the object stands, and the members an earlier `kubectl apply` set that the
    /// declaration no longer has, which a server-side apply leaves.
    async fn state(&mut self, object: Value) -> Result<(State, Vec<Difference>)> {
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
            return Ok((
                State::Absent {
                    note: Some(format!("its namespace {namespace} does not exist yet")),
                },
                Vec::new(),
            ));
        }
        let live = api.get_opt(&name).await.context("cannot read it")?;
        let kind = object["kind"].as_str().unwrap_or_default();
        if kind == "Job" {
            return Ok((
                State::NotCompared(match live {
                    None => "a seed Job runs once and is deleted; plan does not read its claim"
                        .to_owned(),
                    Some(_) => {
                        "a seed Job that was not deleted; plan does not read its claim".to_owned()
                    }
                }),
                Vec::new(),
            ));
        }
        let Some(live) = live else {
            return Ok((State::Absent { note: None }, Vec::new()));
        };
        let live = serde_json::to_value(&live)?;
        let applied: DynamicObject = serde_json::from_value(object.clone())?;
        let params = PatchParams::apply(FIELD_MANAGER).force().dry_run();
        let result = match api.patch(&name, &params, &Patch::Apply(&applied)).await {
            Ok(result) => serde_json::to_value(&result)?,
            Err(kube::Error::Api(status)) => {
                return Ok((State::Refused(status.message.clone()), Vec::new()));
            }
            Err(error) => return Err(anyhow!(error).context("the dry run failed")),
        };
        let leftover = no_longer_declared(&object, &live);
        let mut differences = compare(&normalized(result), &normalized(live.clone()));
        differences.extend(leftover.iter().cloned());
        let state = if differences.is_empty() {
            State::Same
        } else {
            State::Differs {
                differences,
                note: None,
            }
        };
        Ok((state, leftover))
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

impl Cluster {
    /// Forgets what it learned of namespaces, which an earlier step may have created.
    pub fn forget(&mut self) {
        self.namespaces.clear();
    }

    async fn api(&mut self, object: &Value) -> Result<(Api<DynamicObject>, String)> {
        let (resource, capabilities) = self.kind(object).await?;
        let name = object["metadata"]["name"]
            .as_str()
            .context("the object has no name")?
            .to_owned();
        let api = match (
            &capabilities.scope,
            object["metadata"]["namespace"].as_str(),
        ) {
            (Scope::Namespaced, Some(namespace)) => {
                Api::namespaced_with(self.client.clone(), namespace, &resource)
            }
            (Scope::Namespaced, None) => bail!("a namespaced object names no namespace"),
            (Scope::Cluster, _) => Api::all_with(self.client.clone(), &resource),
        };
        Ok((api, name))
    }

    /// Server-side applies the object as `veoveo-installer`, taking over the fields an
    /// earlier `kubectl apply` set, removes the members it no longer declares and waits
    /// for its rollout.
    pub async fn apply_object(&mut self, object: &Value, remove: &[Value]) -> Result<()> {
        let (api, name) = self.api(object).await?;
        let applied: DynamicObject = serde_json::from_value(object.clone())?;
        api.patch(
            &name,
            &PatchParams::apply(FIELD_MANAGER).force(),
            &Patch::Apply(&applied),
        )
        .await
        .map_err(|error| anyhow!("server-side apply of {} failed: {error}", subject(object)))?;
        let params = PatchParams {
            field_manager: Some(FIELD_MANAGER.to_owned()),
            ..PatchParams::default()
        };
        for patch in remove {
            api.patch(&name, &params, &Patch::Merge(patch))
                .await
                .map_err(|error| {
                    anyhow!("removing {patch} from {} failed: {error}", subject(object))
                })?;
        }
        self.await_ready(object, ROLLOUT_TIMEOUT).await
    }

    /// Waits until a workload's rollout is complete; other objects are ready once applied.
    pub async fn await_ready(&mut self, object: &Value, timeout: Duration) -> Result<()> {
        let kind = object["kind"].as_str().unwrap_or_default();
        if !matches!(
            kind,
            "Deployment" | "StatefulSet" | "DaemonSet" | "CustomResourceDefinition"
        ) {
            return Ok(());
        }
        let (api, name) = self.api(object).await?;
        let started = Instant::now();
        loop {
            let live = serde_json::to_value(api.get(&name).await?)?;
            match readiness(&live) {
                Readiness::Ready => return Ok(()),
                Readiness::Waiting(reason) if started.elapsed() > timeout => {
                    let events = self.events(object).await;
                    bail!(
                        "{} is not ready after {} s: {reason}; recent events: {events}",
                        subject(object),
                        timeout.as_secs()
                    );
                }
                Readiness::Waiting(_) => tokio::time::sleep(Duration::from_secs(3)).await,
            }
        }
    }

    /// The last events of an object, for a failure's evidence.
    async fn events(&self, object: &Value) -> String {
        let (Some(namespace), Some(name)) = (
            object["metadata"]["namespace"].as_str(),
            object["metadata"]["name"].as_str(),
        ) else {
            return "none".to_owned();
        };
        let api: Api<k8s_openapi::api::core::v1::Event> =
            Api::namespaced(self.client.clone(), namespace);
        let params = ListParams::default().fields(&format!("involvedObject.name={name}"));
        match api.list(&params).await {
            Ok(list) => {
                let mut events: Vec<_> = list
                    .items
                    .iter()
                    .map(|event| {
                        format!(
                            "{} {}: {}",
                            event.type_.clone().unwrap_or_default(),
                            event.reason.clone().unwrap_or_default(),
                            event.message.clone().unwrap_or_default()
                        )
                    })
                    .collect();
                let keep = events.len().saturating_sub(5);
                events.drain(..keep);
                if events.is_empty() {
                    "none".to_owned()
                } else {
                    events.join(" | ")
                }
            }
            Err(error) => format!("not read: {error}"),
        }
    }

    /// Restarts a workload as `kubectl rollout restart` does and waits for its rollout.
    pub async fn restart(&mut self, workload: &str) -> Result<()> {
        let object = workload_object(workload)?;
        let (api, name) = self.api(&object).await?;
        let patch = json!({"spec": {"template": {"metadata": {"annotations": {
            RESTARTED_AT: chrono_now(),
        }}}}});
        let params = PatchParams {
            field_manager: Some(RESTART_MANAGER.to_owned()),
            ..PatchParams::default()
        };
        api.patch(&name, &params, &Patch::Merge(&patch))
            .await
            .map_err(|error| anyhow!("restarting {workload} failed: {error}"))?;
        self.await_ready(&object, ROLLOUT_TIMEOUT).await
    }

    /// Writes files into Hub's trusted claim: a temporary Secret holds them, the claim's
    /// seed Job copies every key into the claim, and both are deleted after it succeeds.
    pub async fn write_claim(
        &mut self,
        inputs: &Inputs,
        claim: &HubClaim,
        files: &[(String, String)],
    ) -> Result<()> {
        let job = seed_job(inputs, claim)?;
        let namespace = job["metadata"]["namespace"]
            .as_str()
            .context("the seed Job names no namespace")?
            .to_owned();
        let secrets: Api<k8s_openapi::api::core::v1::Secret> =
            Api::namespaced(self.client.clone(), &namespace);
        if secrets.get_opt(&claim.secret).await?.is_some() {
            bail!(
                "Secret {namespace}/{} exists: an earlier seed did not finish; delete it once no seed runs",
                claim.secret
            );
        }
        let secret: k8s_openapi::api::core::v1::Secret = serde_json::from_value(json!({
            "apiVersion": "v1",
            "kind": "Secret",
            "metadata": {"name": claim.secret, "namespace": namespace},
            "type": "Opaque",
            "stringData": files.iter().cloned().collect::<BTreeMap<_, _>>(),
        }))?;
        secrets
            .create(&PostParams::default(), &secret)
            .await
            .context("cannot create the seed's Secret")?;
        let ran = self.run_job(&job).await;
        let deleted = secrets
            .delete(&claim.secret, &DeleteParams::default())
            .await;
        ran?;
        deleted.map_err(|error| {
            anyhow!(
                "the seed's Secret {namespace}/{} was not deleted: {error}",
                claim.secret
            )
        })?;
        Ok(())
    }

    /// Removes files from Hub's trusted claim with a Job made from its seed Job.
    pub async fn remove_from_claim(
        &mut self,
        inputs: &Inputs,
        claim: &HubClaim,
        names: &[String],
    ) -> Result<()> {
        let job = cleanup_job(&seed_job(inputs, claim)?, names)?;
        self.run_job(&job).await
    }

    /// Creates a Job, waits until it succeeds and deletes it; a failed Job stays, as
    /// evidence.
    async fn run_job(&mut self, job: &Value) -> Result<()> {
        let namespace = job["metadata"]["namespace"]
            .as_str()
            .context("the Job names no namespace")?;
        let name = job["metadata"]["name"]
            .as_str()
            .context("the Job has no name")?;
        let jobs: Api<k8s_openapi::api::batch::v1::Job> =
            Api::namespaced(self.client.clone(), namespace);
        if jobs.get_opt(name).await?.is_some() {
            bail!(
                "Job {namespace}/{name} exists: an earlier one did not finish or failed; read it, then delete it"
            );
        }
        let created: k8s_openapi::api::batch::v1::Job = serde_json::from_value(job.clone())?;
        jobs.create(&PostParams::default(), &created)
            .await
            .with_context(|| format!("cannot create Job {namespace}/{name}"))?;
        let started = Instant::now();
        loop {
            let live = serde_json::to_value(jobs.get(name).await?)?;
            match job_outcome(&live) {
                Some(Ok(())) => break,
                Some(Err(reason)) => {
                    bail!("Job {namespace}/{name} failed: {reason}; it is kept as evidence")
                }
                None if started.elapsed() > JOB_TIMEOUT => {
                    bail!(
                        "Job {namespace}/{name} did not finish in {} s; it is kept as evidence",
                        JOB_TIMEOUT.as_secs()
                    )
                }
                None => tokio::time::sleep(Duration::from_secs(2)).await,
            }
        }
        let params = DeleteParams {
            propagation_policy: Some(kube::api::PropagationPolicy::Background),
            ..DeleteParams::default()
        };
        jobs.delete(name, &params)
            .await
            .with_context(|| format!("Job {namespace}/{name} succeeded but was not deleted"))?;
        Ok(())
    }
}

/// Whether a workload's rollout is complete.
#[derive(Debug, PartialEq, Eq)]
pub enum Readiness {
    Ready,
    Waiting(String),
}

/// A workload's readiness from its status, as `kubectl rollout status` reads it.
pub fn readiness(object: &Value) -> Readiness {
    let number = |path: &str| object.pointer(path).and_then(Value::as_i64).unwrap_or(0);
    let generation = number("/metadata/generation");
    if number("/status/observedGeneration") < generation
        && object["kind"] != "CustomResourceDefinition"
    {
        return Readiness::Waiting("the controller has not seen the latest generation".to_owned());
    }
    let waiting = |what: String| Readiness::Waiting(what);
    match object["kind"].as_str().unwrap_or_default() {
        "Deployment" => {
            let wanted = object
                .pointer("/spec/replicas")
                .and_then(Value::as_i64)
                .unwrap_or(1);
            let (updated, ready, available) = (
                number("/status/updatedReplicas"),
                number("/status/readyReplicas"),
                number("/status/availableReplicas"),
            );
            let total = object
                .pointer("/status/replicas")
                .and_then(Value::as_i64)
                .unwrap_or(updated);
            if updated < wanted || total > updated || ready < wanted || available < wanted {
                waiting(format!(
                    "{updated} of {wanted} updated, {ready} ready, {available} available, {total} in all"
                ))
            } else {
                Readiness::Ready
            }
        }
        "StatefulSet" => {
            let wanted = object
                .pointer("/spec/replicas")
                .and_then(Value::as_i64)
                .unwrap_or(1);
            let (updated, ready) = (
                number("/status/updatedReplicas"),
                number("/status/readyReplicas"),
            );
            let current = object.pointer("/status/currentRevision");
            let update = object.pointer("/status/updateRevision");
            if updated < wanted || ready < wanted || (update.is_some() && current != update) {
                waiting(format!("{updated} of {wanted} updated, {ready} ready"))
            } else {
                Readiness::Ready
            }
        }
        "DaemonSet" => {
            let wanted = number("/status/desiredNumberScheduled");
            let (updated, ready) = (
                number("/status/updatedNumberScheduled"),
                number("/status/numberReady"),
            );
            if updated < wanted || ready < wanted {
                waiting(format!("{updated} of {wanted} updated, {ready} ready"))
            } else {
                Readiness::Ready
            }
        }
        "CustomResourceDefinition" => {
            let established = object["status"]["conditions"]
                .as_array()
                .is_some_and(|conditions| {
                    conditions.iter().any(|condition| {
                        condition["type"] == "Established" && condition["status"] == "True"
                    })
                });
            if established {
                Readiness::Ready
            } else {
                waiting("not established".to_owned())
            }
        }
        _ => Readiness::Ready,
    }
}

/// Whether a Job succeeded (`Ok`), failed (`Err` with its reason) or still runs (`None`).
pub fn job_outcome(job: &Value) -> Option<Result<(), String>> {
    let conditions = job["status"]["conditions"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    for condition in &conditions {
        if condition["status"] != "True" {
            continue;
        }
        match condition["type"].as_str() {
            Some("Complete") => return Some(Ok(())),
            Some("Failed") => {
                return Some(Err(format!(
                    "{}: {}",
                    condition["reason"].as_str().unwrap_or_default(),
                    condition["message"].as_str().unwrap_or_default()
                )));
            }
            _ => {}
        }
    }
    if job["status"]["succeeded"].as_i64().unwrap_or(0) > 0 {
        return Some(Ok(()));
    }
    None
}

/// The workload a subject such as `Deployment veoveo-system/lore` names.
pub fn workload_object(subject: &str) -> Result<Value> {
    let (kind, rest) = subject
        .split_once(' ')
        .context("a workload is `<Kind> <namespace>/<name>`")?;
    let (namespace, name) = rest
        .split_once('/')
        .context("a workload is `<Kind> <namespace>/<name>`")?;
    if !matches!(kind, "Deployment" | "StatefulSet" | "DaemonSet") {
        bail!("{kind} is not a workload apply can restart");
    }
    Ok(
        json!({"apiVersion": "apps/v1", "kind": kind, "metadata": {"name": name, "namespace": namespace}}),
    )
}

fn chrono_now() -> String {
    // RFC 3339 in UTC, as kubectl writes the restart annotation.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = now / 86_400;
    let seconds = now % 86_400;
    let (year, month, day) = civil_from_days(days as i64);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        seconds / 3600,
        seconds % 3600 / 60,
        seconds % 60
    )
}

/// The proleptic Gregorian date of a day count since 1970-01-01.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// The claim's seed Job, from its manifest.
pub fn seed_job(inputs: &Inputs, claim: &HubClaim) -> Result<Value> {
    let objects = parse_manifests(&inputs.filled(&claim.manifest)?)?;
    objects
        .into_iter()
        .find(|object| object["kind"] == "Job" && object["metadata"]["name"] == claim.job.as_str())
        .with_context(|| format!("{} holds no Job {}", claim.manifest.display(), claim.job))
}

/// A Job that removes `names` from the claim, made from the claim's seed Job: the same
/// pod, identity and claim, without the Secret.
pub fn cleanup_job(seed: &Value, names: &[String]) -> Result<Value> {
    for name in names {
        if !crate::hub::is_claim_policy(name) {
            bail!("{name:?} is not a file apply may remove");
        }
    }
    let mut job = seed.clone();
    let spec = job
        .pointer_mut("/spec/template/spec")
        .context("the seed Job has no pod")?;
    let volumes = spec["volumes"].as_array().cloned().unwrap_or_default();
    let claim = volumes
        .iter()
        .find(|volume| volume.get("persistentVolumeClaim").is_some())
        .context("the seed Job mounts no claim")?
        .clone();
    let claim_name = claim["name"].clone();
    spec["volumes"] = Value::Array(vec![claim]);
    let container = spec["containers"]
        .get_mut(0)
        .context("the seed Job has no container")?;
    let mount = container["volumeMounts"]
        .as_array()
        .and_then(|mounts| mounts.iter().find(|mount| mount["name"] == claim_name))
        .context("the seed Job does not mount its claim")?
        .clone();
    let directory = mount["mountPath"]
        .as_str()
        .context("the claim's mount has no path")?
        .to_owned();
    container["volumeMounts"] = Value::Array(vec![mount]);
    container["command"] = json!(["/bin/sh", "-ec"]);
    let mut args = vec![
        Value::String("cd \"$0\" && rm -f -- \"$@\"".to_owned()),
        Value::String(directory),
    ];
    args.extend(names.iter().cloned().map(Value::String));
    container["args"] = Value::Array(args);
    let name = format!(
        "{}-cleanup",
        seed["metadata"]["name"].as_str().unwrap_or("claim")
    );
    job["metadata"]["name"] = Value::String(name);
    Ok(job)
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
        // kubectl's own bookkeeping is not a declaration.
        .filter(|difference| difference.path.last_member() != Some(LAST_APPLIED))
        // Only what the object still holds.
        .filter_map(|difference| {
            let active = difference.path.get(live)?.clone();
            Some(Difference::new(difference.path, None, Some(active)))
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
        // The live object still holds the label and the key; the hook section is a
        // changed value, which the dry run shows.
        let live = json!({
            "metadata": {"annotations": {LAST_APPLIED: last.to_string()}, "labels": {"old": "x"}},
            "data": {"local.toml": "[server]\n[hooks]\n", "extra.toml": "x"}
        });
        let found = no_longer_declared(&declared, &live);
        let paths: Vec<String> = found
            .iter()
            .map(|difference| difference.path.to_string())
            .collect();
        assert_eq!(paths, [r#"data["extra.toml"]"#, "metadata.labels"]);
        // Apply removes them with merge patches.
        let patches: Vec<Value> = found
            .iter()
            .map(|difference| difference.path.removal_patch().unwrap())
            .collect();
        assert_eq!(
            patches,
            [
                json!({"data": {"extra.toml": null}}),
                json!({"metadata": {"labels": null}})
            ]
        );
        // Once removed, nothing is left over.
        let live = json!({
            "metadata": {"annotations": {LAST_APPLIED: last.to_string()}},
            "data": {"local.toml": "[server]\n"}
        });
        assert!(no_longer_declared(&declared, &live).is_empty());
    }
}

#[cfg(test)]
mod apply_tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn a_rollout_is_ready_only_when_every_replica_is_updated_and_ready() {
        // Recorded statuses of the Lore Deployment during and after a rollout.
        let mut lore = json!({
            "kind": "Deployment",
            "metadata": {"generation": 13},
            "spec": {"replicas": 1},
            "status": {"observedGeneration": 12, "replicas": 1, "updatedReplicas": 1, "readyReplicas": 1, "availableReplicas": 1}
        });
        assert!(matches!(readiness(&lore), Readiness::Waiting(_)));
        lore["status"] = json!({"observedGeneration": 13, "replicas": 2, "updatedReplicas": 1, "readyReplicas": 1, "availableReplicas": 1});
        assert_eq!(
            readiness(&lore),
            Readiness::Waiting("1 of 1 updated, 1 ready, 1 available, 2 in all".to_owned())
        );
        lore["status"]["replicas"] = json!(1);
        assert_eq!(readiness(&lore), Readiness::Ready);

        let buildkitd = json!({
            "kind": "StatefulSet",
            "metadata": {"generation": 3},
            "spec": {"replicas": 1},
            "status": {"observedGeneration": 3, "updatedReplicas": 1, "readyReplicas": 1, "currentRevision": "buildkitd-6d", "updateRevision": "buildkitd-7f"}
        });
        assert!(matches!(readiness(&buildkitd), Readiness::Waiting(_)));
        let mut done = buildkitd.clone();
        done["status"]["currentRevision"] = json!("buildkitd-7f");
        assert_eq!(readiness(&done), Readiness::Ready);

        let vector = json!({
            "kind": "DaemonSet",
            "metadata": {"generation": 2},
            "status": {"observedGeneration": 2, "desiredNumberScheduled": 1, "updatedNumberScheduled": 0, "numberReady": 1}
        });
        assert!(matches!(readiness(&vector), Readiness::Waiting(_)));
        assert_eq!(readiness(&json!({"kind": "Service"})), Readiness::Ready);
    }

    #[test]
    fn a_job_succeeds_fails_or_still_runs() {
        assert_eq!(job_outcome(&json!({"status": {"active": 1}})), None);
        assert_eq!(
            job_outcome(
                &json!({"status": {"succeeded": 1, "conditions": [{"type": "Complete", "status": "True"}]}})
            ),
            Some(Ok(()))
        );
        assert_eq!(
            job_outcome(
                &json!({"status": {"failed": 1, "conditions": [{"type": "Failed", "status": "True", "reason": "BackoffLimitExceeded", "message": "Job has reached the specified backoff limit"}]}})
            ),
            Some(Err(
                "BackoffLimitExceeded: Job has reached the specified backoff limit".to_owned()
            ))
        );
    }

    #[test]
    fn a_cleanup_job_is_the_seed_job_without_its_secret() {
        let manifest = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../kubernetes/hub-config-pvc.yaml"),
        )
        .unwrap();
        let seed = parse_manifests(&manifest)
            .unwrap()
            .into_iter()
            .find(|object| object["kind"] == "Job")
            .unwrap();
        let names = vec![
            "computer-policy-v1.yaml".to_owned(),
            "factory-base-v2.yaml".to_owned(),
        ];
        let job = cleanup_job(&seed, &names).unwrap();
        assert_eq!(job["metadata"]["name"], "seed-hub-config-v1-cleanup");
        assert_eq!(job["metadata"]["namespace"], seed["metadata"]["namespace"]);
        let pod = &job["spec"]["template"]["spec"];
        assert_eq!(
            pod["securityContext"],
            seed["spec"]["template"]["spec"]["securityContext"]
        );
        assert_eq!(
            pod["volumes"],
            json!([{"name": "output", "persistentVolumeClaim": {"claimName": "hub-config-v1"}}])
        );
        let container = &pod["containers"][0];
        assert_eq!(
            container["volumeMounts"],
            json!([{"name": "output", "mountPath": "/output"}])
        );
        assert_eq!(
            container["args"],
            json!([
                "cd \"$0\" && rm -f -- \"$@\"",
                "/output",
                "computer-policy-v1.yaml",
                "factory-base-v2.yaml"
            ])
        );
        assert!(cleanup_job(&seed, &["../hub.env".to_owned()]).is_err());
        assert!(cleanup_job(&seed, &["hub.env".to_owned()]).is_err());
    }

    #[test]
    fn a_restart_names_a_workload() {
        assert_eq!(
            workload_object("Deployment veoveo-system/lore").unwrap(),
            json!({"apiVersion": "apps/v1", "kind": "Deployment", "metadata": {"name": "lore", "namespace": "veoveo-system"}})
        );
        assert!(workload_object("ConfigMap veoveo-system/lore-settings").is_err());
        assert!(workload_object("lore").is_err());
    }

    #[test]
    fn the_restart_annotation_is_a_utc_timestamp() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20_751), (2026, 10, 25));
        let now = chrono_now();
        assert_eq!(now.len(), "2026-10-25T07:00:00Z".len());
        assert!(now.ends_with('Z'));
    }
}
