//! OpenShell workspaces, provider profiles, templates and sandboxes, read through
//! OpenShell's Rust SDK as the operator's own OpenShell sign-in. Declared policies and
//! profiles go through OpenShell's own parsers, so both sides compare in OpenShell's
//! canonical form.

use std::{collections::BTreeMap, path::PathBuf, time::Duration};

use anyhow::{Context, Result, anyhow, bail};
use openshell_sdk::{
    AuthConfig, ClientConfig, ExecOptions, OpenShellClient,
    oidc::{RefreshTokenInput, refresh_token},
    raw::proto,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    declaration::{Profile, Sandbox, Service, Template},
    difference::{Difference, Path as ValuePath, compare, remove},
    inputs::Inputs,
    plan::{Item, State},
    private::OpenShellAccess,
};

/// A template annotation each publish writes for itself.
pub const RELEASE_NOTE: &str = "veoveo.ai/release-note";

/// The members of `metadata.json` in the CLI's gateway directory that the connection
/// needs (`openshell-bootstrap`'s `GatewayMetadata`).
#[derive(Debug, Deserialize)]
struct GatewayMetadata {
    gateway_endpoint: String,
    #[serde(default)]
    auth_mode: Option<String>,
    #[serde(default)]
    oidc_scopes: Option<String>,
}

/// The CLI's `oidc_token.json` (`openshell-bootstrap`'s `OidcTokenBundle`).
#[derive(Deserialize)]
struct OidcTokenBundle {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_at: Option<u64>,
    issuer: String,
    client_id: String,
}

/// The gateway and how the operator signs in to it.
pub struct Gateway {
    pub endpoint: String,
    pub client: OpenShellClient,
}

/// Connects as the operator's OpenShell CLI sign-in. An expired access token is
/// renewed with the stored refresh token in memory; the CLI's files are not changed.
pub async fn connect(access: &OpenShellAccess) -> Result<Gateway> {
    let home = match &access.config_home {
        Some(home) => home.clone(),
        None => std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
            .context("neither XDG_CONFIG_HOME nor HOME is set")?,
    };
    let directory = home
        .join("openshell")
        .join("gateways")
        .join(&access.gateway);
    let metadata: GatewayMetadata = read_json(&directory.join("metadata.json"))?;
    let mut config = ClientConfig::new(metadata.gateway_endpoint.clone());
    let ca = directory.join("mtls").join("ca.crt");
    if ca.exists() {
        config.ca_cert =
            Some(std::fs::read(&ca).with_context(|| format!("cannot read {}", ca.display()))?);
    }
    if metadata.auth_mode.as_deref() == Some("oidc") {
        let bundle: OidcTokenBundle = read_json(&directory.join("oidc_token.json"))
            .with_context(|| format!("gateway {} has no OpenShell sign-in", access.gateway))?;
        let (token, expires_at) = fresh_token(bundle, metadata.oidc_scopes.as_deref()).await?;
        config.auth = Some(AuthConfig::Oidc {
            token,
            expires_at,
            refresh: None,
        });
    }
    let client = OpenShellClient::connect(config).await.with_context(|| {
        format!(
            "cannot reach the OpenShell gateway at {}",
            metadata.gateway_endpoint
        )
    })?;
    client.health().await.with_context(|| {
        format!(
            "the OpenShell gateway at {} does not answer",
            metadata.gateway_endpoint
        )
    })?;
    Ok(Gateway {
        endpoint: metadata.gateway_endpoint,
        client,
    })
}

fn read_json<T: serde::de::DeserializeOwned>(path: &std::path::Path) -> Result<T> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("{} is not valid", path.display()))
}

async fn fresh_token(
    bundle: OidcTokenBundle,
    scopes: Option<&str>,
) -> Result<(String, Option<u64>)> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    if bundle
        .expires_at
        .is_none_or(|expires_at| expires_at > now + 60)
    {
        return Ok((bundle.access_token, bundle.expires_at));
    }
    let refresh = bundle.refresh_token.context(
        "the OpenShell sign-in expired and holds no refresh token; sign in with the OpenShell CLI",
    )?;
    let input = RefreshTokenInput::new(refresh, bundle.issuer, bundle.client_id)
        .with_scopes(scopes.unwrap_or_default().split_whitespace());
    let output = refresh_token(&input).await.map_err(|error| {
        anyhow!(
            "the OpenShell sign-in could not be renewed: {error}; sign in with the OpenShell CLI"
        )
    })?;
    Ok((output.access_token, output.expires_at))
}

fn not_found(status: &tonic::Status) -> bool {
    status.code() == tonic::Code::NotFound
}

impl Gateway {
    pub async fn plan_workspaces(&self, workspaces: &[String]) -> Vec<Item> {
        let mut items = Vec::new();
        for name in workspaces {
            let subject = format!("workspace {name}");
            let request = proto::GetWorkspaceRequest { name: name.clone() };
            items.push(match self.client.raw_grpc().get_workspace(request).await {
                Ok(_) => Item::new(subject, State::Same),
                Err(status) if not_found(&status) => {
                    Item::new(subject, State::Absent { note: None })
                }
                Err(status) => Item::unread(subject, status.message()),
            });
        }
        items
    }

    pub async fn plan_profiles(&self, inputs: &Inputs, profiles: &[Profile]) -> Vec<Item> {
        let mut items = Vec::new();
        for profile in profiles {
            items.push(self.plan_profile(inputs, profile).await);
        }
        items
    }

    async fn plan_profile(&self, inputs: &Inputs, profile: &Profile) -> Item {
        let declared = match inputs
            .text(&profile.file)
            .and_then(|text| declared_profile(&text))
        {
            Ok(declared) => declared,
            Err(error) => {
                return Item::new(
                    profile.file.display().to_string(),
                    State::NotCompared(format!("{error:#}")),
                );
            }
        };
        let id = declared["id"].as_str().unwrap_or_default().to_owned();
        let subject = match &profile.workspace {
            Some(workspace) => format!("profile {workspace}/{id}"),
            None => format!("global profile {id}"),
        };
        let request = proto::GetProviderProfileRequest {
            id: id.clone(),
            workspace_scope: profile.workspace.as_ref().map(proto::workspace_selector),
        };
        let live = match self.client.raw_grpc().get_provider_profile(request).await {
            Ok(response) => match response.into_inner().profile {
                Some(live) => live,
                None => return Item::unread(subject, "the gateway answered no profile"),
            },
            Err(status) if not_found(&status) => {
                return Item::new(subject, State::Absent { note: None });
            }
            Err(status) => return Item::unread(subject, status.message()),
        };
        let live = active_profile(&live);
        let mut differences = profile_scope(profile, &live);
        let mut declared = declared;
        let mut live = live;
        for value in [&mut declared, &mut live] {
            for member in ["resource_version", "source", "scope"] {
                remove(value, member);
            }
        }
        differences.extend(compare(&declared, &live));
        Item::compared(subject, differences, None)
    }

    pub async fn plan_templates(&self, inputs: &Inputs, templates: &[Template]) -> Vec<Item> {
        let mut items = Vec::new();
        for template in templates {
            items.push(self.plan_template(inputs, template).await);
        }
        items
    }

    async fn plan_template(&self, inputs: &Inputs, template: &Template) -> Item {
        let subject = format!("template {}/{}", template.workspace, template.name);
        let declared = match declared_template(inputs, template) {
            Ok(declared) => declared,
            Err(error) => return Item::new(subject, State::NotCompared(format!("{error:#}"))),
        };
        let request = proto::GetSandboxTemplateRequest {
            name: template.name.clone(),
            workspace_scope: Some(proto::workspace_selector(&template.workspace)),
        };
        let live = match self.client.raw_grpc().get_sandbox_template(request).await {
            Ok(response) => match response.into_inner().template {
                Some(live) => live,
                None => return Item::unread(subject, "the gateway answered no template"),
            },
            Err(status) if not_found(&status) => {
                return Item::new(subject, State::Absent { note: None });
            }
            Err(status) => return Item::unread(subject, status.message()),
        };
        Item::compared(
            subject,
            compare(&declared, &active_template(&live)),
            Some(
                "OpenShell has no template update: apply deletes the template and creates it again"
                    .to_owned(),
            ),
        )
    }

    pub async fn plan_sandboxes(&self, inputs: &Inputs, sandboxes: &[Sandbox]) -> Vec<Item> {
        let mut items = Vec::new();
        for sandbox in sandboxes {
            items.extend(self.plan_sandbox(inputs, sandbox).await);
        }
        items
    }

    async fn plan_sandbox(&self, inputs: &Inputs, sandbox: &Sandbox) -> Vec<Item> {
        let subject = format!("sandbox {}", sandbox.key());
        let declared = match declared_sandbox(inputs, sandbox) {
            Ok(declared) => declared,
            Err(error) => {
                return vec![Item::new(subject, State::NotCompared(format!("{error:#}")))];
            }
        };
        let request = proto::GetSandboxRequest {
            name: sandbox.name.clone(),
            workspace_scope: Some(proto::workspace_selector(&sandbox.workspace)),
        };
        let live = match self.client.raw_grpc().get_sandbox(request).await {
            Ok(response) => match response.into_inner().sandbox {
                Some(live) => live,
                None => return vec![Item::unread(subject, "the gateway answered no sandbox")],
            },
            Err(status) if not_found(&status) => {
                return vec![Item::new(subject, State::Absent { note: None })];
            }
            Err(status) => return vec![Item::unread(subject, status.message())],
        };
        let active = match active_sandbox(&live) {
            Ok(active) => active,
            Err(error) => return vec![Item::unread(subject, format!("{error:#}"))],
        };
        let mut items = vec![Item::compared(
            subject,
            compare(&declared, &active),
            Some("OpenShell has no sandbox update: apply stops, deletes and creates it again, keeping its claims and providers".to_owned()),
        )];
        if !sandbox.services.is_empty() {
            items.extend(self.plan_services(sandbox).await);
        }
        items
    }

    async fn plan_services(&self, sandbox: &Sandbox) -> Vec<Item> {
        let request = proto::ListServicesRequest {
            workspace_scope: Some(proto::workspace_selector(&sandbox.workspace)),
            page_size: 100,
            page_token: String::new(),
            sandbox: sandbox.name.clone(),
        };
        let listed = match self.client.raw_grpc().list_services(request).await {
            Ok(response) => response.into_inner().services,
            Err(status) => {
                return vec![Item::unread(
                    format!("services of sandbox {}", sandbox.key()),
                    status.message(),
                )];
            }
        };
        let active: Vec<Value> = listed
            .iter()
            .filter_map(|service| service.endpoint.as_ref())
            .map(active_service)
            .collect();
        sandbox
            .services
            .iter()
            .map(|service| {
                let subject = format!(
                    "service {}/{}:{}",
                    sandbox.key(),
                    service.name,
                    service.port
                );
                let declared = declared_service(service);
                match active
                    .iter()
                    .find(|active| active["name"] == declared["name"])
                {
                    Some(active) => Item::compared(subject, compare(&declared, active), None),
                    None => Item::new(subject, State::Absent { note: None }),
                }
            })
            .collect()
    }

    /// A file in a sandbox, read with `cat` through native exec.
    pub async fn read_file(&self, workspace: &str, sandbox: &str, path: &str) -> Result<String> {
        let output = self.exec(workspace, sandbox, &["/bin/cat", path]).await?;
        Ok(output)
    }

    /// The names in a sandbox directory.
    pub async fn list_directory(
        &self,
        workspace: &str,
        sandbox: &str,
        path: &str,
    ) -> Result<Vec<String>> {
        let output = self
            .exec(workspace, sandbox, &["/bin/ls", "-1", path])
            .await?;
        Ok(output
            .lines()
            .map(str::to_owned)
            .filter(|name| !name.is_empty())
            .collect())
    }

    async fn exec(&self, workspace: &str, sandbox: &str, command: &[&str]) -> Result<String> {
        let options = ExecOptions {
            no_login_shell: true,
            timeout: Some(Duration::from_secs(30)),
            ..ExecOptions::default()
        };
        let command: Vec<String> = command.iter().map(|part| (*part).to_owned()).collect();
        let result = self
            .client
            .workspace(workspace)
            .exec(sandbox, &command, options)
            .await
            .map_err(|error| anyhow!("{error}"))?;
        if result.exit_code != 0 {
            bail!(
                "{} exited {} in {workspace}/{sandbox}: {}",
                command.join(" "),
                result.exit_code,
                String::from_utf8_lossy(&result.stderr).trim()
            );
        }
        Ok(String::from_utf8(result.stdout)?)
    }
}

/// A declared profile in OpenShell's canonical form: parsed as `profile import` parses
/// it, and through the protobuf form the gateway stores.
pub fn declared_profile(text: &str) -> Result<Value> {
    let parsed =
        openshell_providers::parse_profile_yaml(text).map_err(|error| anyhow!("{error}"))?;
    let canonical = openshell_providers::ProviderTypeProfile::from_proto(&parsed.to_proto());
    Ok(serde_json::to_value(canonical)?)
}

pub fn active_profile(profile: &proto::ProviderProfile) -> Value {
    serde_json::to_value(openshell_providers::ProviderTypeProfile::from_proto(
        profile,
    ))
    .unwrap_or(Value::Null)
}

/// A global profile is stored in the platform scope, a workspace profile in its
/// workspace; a lookup in a workspace also finds a platform profile of the same id.
fn profile_scope(profile: &Profile, live: &Value) -> Vec<Difference> {
    let declared = if profile.workspace.is_some() {
        "workspace"
    } else {
        "platform"
    };
    match live["scope"].as_str() {
        Some(scope) if scope != declared => vec![Difference::new(
            ValuePath::root().member("scope"),
            Some(Value::String(declared.to_owned())),
            Some(Value::String(scope.to_owned())),
        )],
        _ => Vec::new(),
    }
}

fn declared_template(inputs: &Inputs, template: &Template) -> Result<Value> {
    let image = template
        .image
        .as_deref()
        .map(|name| inputs.image(name))
        .transpose()?;
    let driver_config = match &template.driver_config {
        Some(file) => serde_json::from_str::<Value>(&inputs.filled(file)?)
            .with_context(|| format!("{} is not JSON", file.display()))?,
        None => Value::Null,
    };
    Ok(json!({
        "image": image.unwrap_or_default(),
        "environment": template.environment,
        "resources": {"cpu": template.cpu.clone().unwrap_or_default(), "memory": template.memory.clone().unwrap_or_default()},
        "driver_config": driver_config,
        "labels": {},
        "annotations": {},
    }))
}

pub fn active_template(template: &proto::SandboxWorkloadTemplate) -> Value {
    let metadata = template.metadata.clone().unwrap_or_default();
    let spec = template.spec.clone().unwrap_or_default();
    let workload = spec.workload.unwrap_or_default();
    let resources = workload.resources.unwrap_or_default();
    let mut annotations: BTreeMap<String, String> = metadata.annotations.into_iter().collect();
    annotations.remove(RELEASE_NOTE);
    let mut value = json!({
        "image": workload.image,
        "environment": workload.environment.into_iter().collect::<BTreeMap<_, _>>(),
        "resources": {"cpu": resources.cpu, "memory": resources.memory},
        "driver_config": spec.driver_config.as_ref().map_or(Value::Null, struct_to_json),
        "labels": metadata.labels.into_iter().collect::<BTreeMap<_, _>>(),
        "annotations": annotations,
    });
    if let Some(gpu) = resources.gpu.and_then(|gpu| gpu.count) {
        value["resources"]["gpu"] = json!(gpu);
    }
    value
}

fn declared_sandbox(inputs: &Inputs, sandbox: &Sandbox) -> Result<Value> {
    let image = inputs.image(&sandbox.image)?;
    let policy = declared_policy(&inputs.filled(&sandbox.policy)?)
        .with_context(|| format!("{} is not a valid policy", sandbox.policy.display()))?;
    let driver_config = match &sandbox.driver_config {
        Some(file) => serde_json::from_str::<Value>(&inputs.filled(file)?)
            .with_context(|| format!("{} is not JSON", file.display()))?,
        None => Value::Null,
    };
    let mut environment = sandbox.environment.clone();
    if !sandbox.private_environment.is_empty() {
        let private = inputs.private.sandbox_environment(&sandbox.key());
        for key in &sandbox.private_environment {
            let value = private
                .and_then(|values| values.get(key))
                .with_context(|| {
                    format!("private.toml sets no {key} for sandbox {}", sandbox.key())
                })?;
            environment.insert(key.clone(), value.clone());
        }
    }
    let mut providers = sandbox.providers.clone();
    providers.sort();
    Ok(json!({
        "image": image,
        "command": sandbox.command,
        "environment": environment,
        "providers": providers,
        "restart_policy": sandbox.restart_policy.as_str(),
        "resources": resource_limits(sandbox.cpu.as_deref(), sandbox.memory.as_deref()),
        "gpu": sandbox.gpu,
        "driver_config": driver_config,
        "policy": policy,
    }))
}

/// A declared policy as the gateway admits it: parsed, validated and canonicalized.
pub fn declared_policy(text: &str) -> Result<Value> {
    let parsed =
        openshell_policy::parse_sandbox_policy(text).map_err(|error| anyhow!("{error}"))?;
    let canonical = openshell_policy::validate_and_canonicalize_sandbox_policy(parsed)
        .map_err(|error| anyhow!("{error}"))?;
    openshell_policy::sandbox_policy_to_json_value(&canonical).map_err(|error| anyhow!("{error}"))
}

/// A sandbox's base policy: its admitted policy without the rules its providers add.
pub fn active_policy(policy: &proto::SandboxPolicy) -> Result<Value> {
    let mut base = policy.clone();
    openshell_policy::strip_provider_rule_names(&mut base);
    openshell_policy::sandbox_policy_to_json_value(&base).map_err(|error| anyhow!("{error}"))
}

pub fn active_sandbox(sandbox: &proto::Sandbox) -> Result<Value> {
    let spec = sandbox.spec.clone().unwrap_or_default();
    let template = spec.template.clone().unwrap_or_default();
    let mut environment: BTreeMap<String, String> = template.environment.into_iter().collect();
    environment.extend(spec.environment.clone());
    let mut providers = spec.providers.clone();
    providers.sort();
    let restart_policy = match spec.restart_policy() {
        proto::SandboxRestartPolicy::Always => "always",
        proto::SandboxRestartPolicy::OnFailure => "on-failure",
        proto::SandboxRestartPolicy::Never => "never",
        proto::SandboxRestartPolicy::Unspecified => "",
    };
    let gpu = spec
        .resource_requirements
        .as_ref()
        .and_then(|requirements| requirements.gpu.as_ref())
        .and_then(|gpu| gpu.count);
    let policy = match &spec.policy {
        Some(policy) => active_policy(policy)?,
        None => Value::Null,
    };
    Ok(json!({
        "image": template.image,
        "command": spec.command,
        "environment": environment,
        "providers": providers,
        "restart_policy": restart_policy,
        "resources": template.resources.as_ref().map_or(Value::Null, struct_to_json),
        "gpu": gpu,
        "driver_config": template.driver_config.as_ref().map_or(Value::Null, struct_to_json),
        "policy": policy,
    }))
}

/// A sandbox's CPU and memory limits as `sandbox create --cpu --memory` sets them.
fn resource_limits(cpu: Option<&str>, memory: Option<&str>) -> Value {
    let mut limits = serde_json::Map::new();
    if let Some(cpu) = cpu {
        limits.insert("cpu".to_owned(), json!(cpu));
    }
    if let Some(memory) = memory {
        limits.insert("memory".to_owned(), json!(memory));
    }
    if limits.is_empty() {
        Value::Null
    } else {
        json!({"limits": limits})
    }
}

fn declared_service(service: &Service) -> Value {
    json!({"name": service.name, "port": service.port, "authorization": service.authorization})
}

pub fn active_service(endpoint: &proto::ServiceEndpoint) -> Value {
    let authorization = match endpoint.authorization_mode() {
        proto::ServiceAuthorizationMode::BearerPassthrough => "bearer-passthrough",
        proto::ServiceAuthorizationMode::Strip | proto::ServiceAuthorizationMode::Unspecified => {
            "strip"
        }
    };
    json!({"name": endpoint.name, "port": endpoint.target_port, "authorization": authorization})
}

/// A protobuf `Struct` as JSON; whole numbers stay integers.
pub fn struct_to_json(value: &prost_types::Struct) -> Value {
    Value::Object(
        value
            .fields
            .iter()
            .map(|(name, field)| (name.clone(), value_to_json(field)))
            .collect(),
    )
}

fn value_to_json(value: &prost_types::Value) -> Value {
    use prost_types::value::Kind;
    match &value.kind {
        None | Some(Kind::NullValue(_)) => Value::Null,
        Some(Kind::BoolValue(value)) => Value::Bool(*value),
        Some(Kind::NumberValue(number)) => {
            if number.fract() == 0.0 && number.abs() < 9.0e15 {
                json!(*number as i64)
            } else {
                json!(number)
            }
        }
        Some(Kind::StringValue(text)) => Value::String(text.clone()),
        Some(Kind::ListValue(list)) => {
            Value::Array(list.values.iter().map(value_to_json).collect())
        }
        Some(Kind::StructValue(inner)) => struct_to_json(inner),
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    const PROFILE: &str = r#"
id: vv-lore
display_name: Lore access as a person
credentials:
  - name: access_token
    env_vars: [LORE_ACCESS_TOKEN]
    required: true
    auth_style: bearer
    header_name: authorization
endpoints:
  - host: lore.veoveo-system.svc.cluster.local
    port: 41337
    protocol: rest
    enforcement: enforce
    rules:
      - allow: {method: POST, path: /**}
binaries:
  - /opt/veoveo/lore/usr/local/bin/lore
"#;

    /// The profile as `openshell profile export` writes it, with the defaults and the
    /// stored members the gateway adds (recorded from the reference gateway).
    const EXPORTED: &str = r#"
id: vv-lore
resource_version: 2
display_name: Lore access as a person
description: ""
category: other
credentials:
  - name: access_token
    description: ""
    env_vars:
      - LORE_ACCESS_TOKEN
    required: true
    auth_style: bearer
    header_name: authorization
    query_param: ""
endpoints:
  - host: lore.veoveo-system.svc.cluster.local
    port: 41337
    protocol: rest
    enforcement: enforce
    rules:
      - allow:
          method: POST
          path: /**
binaries:
  - /opt/veoveo/lore/usr/local/bin/lore
inference_capable: false
source: user
scope: platform
"#;

    fn stored(text: &str) -> proto::ProviderProfile {
        openshell_providers::parse_profile_yaml(text)
            .unwrap()
            .to_proto()
    }

    #[test]
    fn a_profile_and_its_export_are_the_same_profile() {
        let declared = declared_profile(PROFILE).unwrap();
        let mut active = active_profile(&stored(EXPORTED));
        let mut declared_copy = declared.clone();
        for value in [&mut declared_copy, &mut active] {
            for member in ["resource_version", "source", "scope"] {
                remove(value, member);
            }
        }
        assert_eq!(compare(&declared_copy, &active), vec![]);
    }

    #[test]
    fn a_changed_binary_is_a_profile_difference() {
        let declared = declared_profile(PROFILE).unwrap();
        let active = active_profile(&stored(
            &EXPORTED.replace("/opt/veoveo/lore/usr/local/bin/lore", "/usr/local/bin/lore"),
        ));
        let paths: Vec<String> = compare(&declared, &active)
            .iter()
            .map(|difference| difference.path.to_string())
            .filter(|path| !matches!(path.as_str(), "resource_version" | "source" | "scope"))
            .collect();
        assert_eq!(paths, ["binaries[0]"]);
    }

    #[test]
    fn a_global_profile_found_in_a_workspace_scope_is_a_difference() {
        let profile = Profile {
            file: "p.yaml".into(),
            workspace: Some("vv-services".into()),
        };
        let differences = profile_scope(&profile, &json!({"scope": "platform"}));
        assert_eq!(differences.len(), 1);
        assert!(profile_scope(&profile, &json!({"scope": "workspace"})).is_empty());
    }

    #[test]
    fn provider_rules_are_not_part_of_the_base_policy() {
        let text = "version: 1\nfilesystem_policy:\n  include_workdir: false\n  read_only: [/usr]\n  read_write: [/tmp]\nnetwork_policies:\n  hub:\n    name: hub\n    endpoints:\n      - {host: hub.example, port: 443}\n    binaries:\n      - {path: /usr/bin/curl}\n";
        let declared = declared_policy(text).unwrap();
        let mut admitted = openshell_policy::parse_sandbox_policy(text).unwrap();
        let mut provider = admitted.network_policies["hub"].clone();
        provider.name = "_provider_vv_console_registration".into();
        admitted
            .network_policies
            .insert("_provider_vv_console_registration".into(), provider);
        assert_eq!(
            compare(&declared, &active_policy(&admitted).unwrap()),
            vec![]
        );
    }

    #[test]
    fn a_driver_configuration_struct_reads_back_as_its_json() {
        let declared = json!({"kubernetes": {
            "volumes": [{"name": "lore", "image": {"reference": "r@sha256:1"}}],
            "replicas": 1,
            "ratio": 0.5,
            "on": true,
            "none": null,
        }});
        let Some(prost_types::value::Kind::StructValue(as_struct)) = to_protobuf(&declared).kind
        else {
            panic!("an object is a Struct");
        };
        assert_eq!(struct_to_json(&as_struct), declared);
    }

    fn to_protobuf(value: &Value) -> prost_types::Value {
        use prost_types::value::Kind;
        let kind = match value {
            Value::Null => Kind::NullValue(0),
            Value::Bool(value) => Kind::BoolValue(*value),
            Value::Number(number) => Kind::NumberValue(number.as_f64().unwrap()),
            Value::String(text) => Kind::StringValue(text.clone()),
            Value::Array(items) => Kind::ListValue(prost_types::ListValue {
                values: items.iter().map(to_protobuf).collect(),
            }),
            Value::Object(members) => Kind::StructValue(prost_types::Struct {
                fields: members
                    .iter()
                    .map(|(name, member)| (name.clone(), to_protobuf(member)))
                    .collect(),
            }),
        };
        prost_types::Value { kind: Some(kind) }
    }

    #[test]
    fn a_sandbox_reads_back_as_its_creation_inputs() {
        // A recorded Media sandbox as GetSandbox answers it: providers in attachment
        // order, the CLI's --cpu and --memory as template limits.
        let mut limits = prost_types::Struct::default();
        limits.fields.insert(
            "limits".to_owned(),
            to_protobuf(&json!({"cpu": "1", "memory": "1Gi"})),
        );
        let sandbox = proto::Sandbox {
            spec: Some(proto::SandboxSpec {
                environment: [("VEOVEO_PORT".to_owned(), "8802".to_owned())]
                    .into_iter()
                    .collect(),
                template: Some(proto::SandboxTemplate {
                    image: "media@sha256:1".into(),
                    resources: Some(limits),
                    ..Default::default()
                }),
                providers: vec!["vv-media-registration".into(), "vv-media-generation".into()],
                command: vec!["/usr/local/bin/veoveo-mcp-media".into()],
                restart_policy: proto::SandboxRestartPolicy::Always as i32,
                ..Default::default()
            }),
            ..Default::default()
        };
        let declared = json!({
            "image": "media@sha256:1",
            "command": ["/usr/local/bin/veoveo-mcp-media"],
            "environment": {"VEOVEO_PORT": "8802"},
            "providers": ["vv-media-generation", "vv-media-registration"],
            "restart_policy": "always",
            "resources": resource_limits(Some("1"), Some("1Gi")),
            "gpu": null,
            "driver_config": null,
            "policy": null,
        });
        assert_eq!(
            compare(&declared, &active_sandbox(&sandbox).unwrap()),
            vec![]
        );
        let declared = json!({"restart_policy": "on-failure", "resources": null});
        let paths: Vec<String> = compare(&declared, &active_sandbox(&sandbox).unwrap())
            .iter()
            .map(|difference| difference.path.to_string())
            .filter(|path| path == "restart_policy" || path == "resources")
            .collect();
        assert_eq!(paths, ["resources", "restart_policy"]);
    }

    #[test]
    fn a_template_keeps_its_release_note_out_of_the_comparison() {
        let template = proto::SandboxWorkloadTemplate {
            metadata: Some(proto::ObjectMeta {
                name: "computer-v1".into(),
                annotations: [(RELEASE_NOTE.to_owned(), "What is new".to_owned())]
                    .into_iter()
                    .collect(),
                ..Default::default()
            }),
            spec: Some(proto::SandboxWorkloadTemplateSpec {
                workload: Some(proto::SandboxWorkloadConfig {
                    image: "computer@sha256:1".into(),
                    environment: [("SSL_CERT_DIR".to_owned(), "/etc/ssl/certs".to_owned())]
                        .into_iter()
                        .collect(),
                    resources: Some(proto::SandboxResources {
                        cpu: "2".into(),
                        memory: "4Gi".into(),
                        gpu: None,
                    }),
                }),
                driver_config: None,
                desired_service_level: None,
            }),
        };
        let declared = json!({
            "image": "computer@sha256:1",
            "environment": {"SSL_CERT_DIR": "/etc/ssl/certs"},
            "resources": {"cpu": "2", "memory": "4Gi"},
            "driver_config": null,
            "labels": {},
            "annotations": {},
        });
        assert_eq!(compare(&declared, &active_template(&template)), vec![]);
    }
}
