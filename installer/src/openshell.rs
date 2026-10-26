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
    action::Action,
    declaration::{Profile, Sandbox, Service, Template},
    difference::{Difference, Path as ValuePath, compare, remove},
    inputs::Inputs,
    plan::{Item, State},
    private::OpenShellAccess,
};

/// The template annotation that holds its version's release note, which
/// `installation.toml` declares beside the template.
pub const RELEASE_NOTE: &str = "veoveo.ai/release-note";

/// How long a sandbox may take to stop or to be deleted.
const STOP_TIMEOUT: Duration = Duration::from_secs(300);
/// How long a sandbox may take to become ready; a GPU runtime loads its model first.
const READY_TIMEOUT: Duration = Duration::from_secs(1200);

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
                        .with_action(Action::CreateWorkspace(name.clone()))
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
        let stored = match inputs.text(&profile.file).and_then(|text| {
            openshell_providers::parse_profile_yaml(&text)
                .map(|parsed| parsed.to_proto())
                .map_err(|error| anyhow!("{error}"))
        }) {
            Ok(stored) => stored,
            Err(error) => return Item::unusable(profile.file.display().to_string(), &error),
        };
        let source = profile.file.display().to_string();
        // Through the protobuf form the gateway stores, so defaults compare as stored.
        let declared = active_profile(&stored);
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
                return Item::new(subject, State::Absent { note: None }).with_action(
                    Action::ImportProfile {
                        workspace: profile.workspace.clone(),
                        profile: Box::new(stored),
                        source,
                    },
                );
            }
            Err(status) => return Item::unread(subject, status.message()),
        };
        let resource_version = live.resource_version;
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
        let action = if live["scope"].as_str() == Some("platform") && profile.workspace.is_some() {
            // Only a platform profile of this id exists: the workspace's is imported.
            Action::ImportProfile {
                workspace: profile.workspace.clone(),
                profile: Box::new(stored),
                source,
            }
        } else {
            Action::UpdateProfile {
                workspace: profile.workspace.clone(),
                profile: Box::new(stored),
                source,
                resource_version,
            }
        };
        Item::compared(subject, differences, None).with_action(action)
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
        let declared = match template_proto(inputs, template) {
            Ok(declared) => declared,
            Err(error) => return Item::unusable(subject, &error),
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
                return Item::new(subject, State::Absent { note: None }).with_action(
                    Action::ReplaceTemplate {
                        workspace: template.workspace.clone(),
                        template: Box::new(declared),
                        exists: false,
                    },
                );
            }
            Err(status) => return Item::unread(subject, status.message()),
        };
        compare_template(subject, &template.workspace, declared, &live)
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
        let (create, declared) = match sandbox_request(inputs, sandbox)
            .and_then(|create| requested_sandbox(&create).map(|declared| (create, declared)))
        {
            Ok(declared) => declared,
            Err(error) => return vec![Item::unusable(subject, &error)],
        };
        let replace = |exists| Action::ReplaceSandbox {
            workspace: sandbox.workspace.clone(),
            name: sandbox.name.clone(),
            request: Box::new(create.clone()),
            services: sandbox.services.clone(),
            exists,
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
                let mut items = vec![
                    Item::new(subject, State::Absent { note: None }).with_action(replace(false)),
                ];
                items.extend(sandbox.services.iter().map(|service| {
                    Item::new(
                        service_subject(sandbox, service),
                        State::Absent { note: None },
                    )
                    .with_action(expose(sandbox, service, false))
                }));
                return items;
            }
            Err(status) => return vec![Item::unread(subject, status.message())],
        };
        let active = match active_sandbox(&live) {
            Ok(active) => active,
            Err(error) => return vec![Item::unread(subject, format!("{error:#}"))],
        };
        let mut items = vec![
            Item::compared(subject, compare(&declared, &active), None).with_action(replace(true)),
        ];
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
                let subject = service_subject(sandbox, service);
                let declared = declared_service(service);
                match active
                    .iter()
                    .find(|active| active["name"] == declared["name"])
                {
                    Some(active) => Item::compared(subject, compare(&declared, active), None)
                        .with_action(expose(sandbox, service, true)),
                    None => Item::new(subject, State::Absent { note: None })
                        .with_action(expose(sandbox, service, false)),
                }
            })
            .collect()
    }

    /// Whether the sandbox exists.
    pub async fn sandbox_exists(&self, workspace: &str, name: &str) -> Result<bool> {
        Ok(self.phase(workspace, name).await?.is_some())
    }

    /// The sandbox's phase, `None` when it does not exist.
    pub async fn phase(&self, workspace: &str, name: &str) -> Result<Option<proto::SandboxPhase>> {
        let request = proto::GetSandboxRequest {
            name: name.to_owned(),
            workspace_scope: Some(proto::workspace_selector(workspace)),
        };
        match self.client.raw_grpc().get_sandbox(request).await {
            Ok(response) => Ok(response
                .into_inner()
                .sandbox
                .map(|sandbox| sandbox.status.unwrap_or_default().phase())),
            Err(status) if not_found(&status) => Ok(None),
            Err(status) => bail!(
                "cannot read sandbox {workspace}/{name}: {}",
                status.message()
            ),
        }
    }

    /// Waits until the sandbox's phase is `want`, or it is gone when `want` is `None`.
    pub async fn await_phase(
        &self,
        workspace: &str,
        name: &str,
        want: Option<proto::SandboxPhase>,
        timeout: Duration,
    ) -> Result<()> {
        let started = std::time::Instant::now();
        loop {
            let phase = self.phase(workspace, name).await?;
            if phase == want {
                return Ok(());
            }
            if phase == Some(proto::SandboxPhase::Error) && want.is_some() {
                bail!("{}", self.evidence(workspace, name).await);
            }
            if started.elapsed() > timeout {
                bail!(
                    "{workspace}/{name} is {} after {} s, not {}; {}",
                    phase.map_or("absent", |phase| phase.as_str_name()),
                    timeout.as_secs(),
                    want.map_or("absent", |phase| phase.as_str_name()),
                    self.evidence(workspace, name).await
                );
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }

    /// The sandbox's phase and conditions, for a failure's evidence.
    pub async fn evidence(&self, workspace: &str, name: &str) -> String {
        let request = proto::GetSandboxRequest {
            name: name.to_owned(),
            workspace_scope: Some(proto::workspace_selector(workspace)),
        };
        match self.client.raw_grpc().get_sandbox(request).await {
            Ok(response) => {
                let status = response
                    .into_inner()
                    .sandbox
                    .and_then(|sandbox| sandbox.status);
                match status {
                    Some(status) => format!(
                        "phase {}; conditions: {}",
                        status.phase().as_str_name(),
                        status
                            .conditions
                            .iter()
                            .map(|condition| format!(
                                "{}={} ({}: {})",
                                condition.r#type,
                                condition.status,
                                condition.reason,
                                condition.message
                            ))
                            .collect::<Vec<_>>()
                            .join("; ")
                    ),
                    None => "no status".to_owned(),
                }
            }
            Err(status) => format!("its status could not be read: {}", status.message()),
        }
    }

    pub async fn create_workspace(&self, name: &str) -> Result<()> {
        let request = proto::CreateWorkspaceRequest {
            name: name.to_owned(),
            ..Default::default()
        };
        self.client
            .raw_grpc()
            .create_workspace(request)
            .await
            .map_err(|status| anyhow!("CreateWorkspace {name}: {}", status.message()))?;
        Ok(())
    }

    /// Lints the profile, then imports it or updates it at `resource_version`.
    pub async fn put_profile(
        &self,
        workspace: Option<&str>,
        profile: &proto::ProviderProfile,
        source: &str,
        resource_version: Option<u64>,
    ) -> Result<()> {
        let scope = workspace.map(proto::workspace_selector);
        let item = proto::ProviderProfileImportItem {
            profile: Some(profile.clone()),
            source: source.to_owned(),
        };
        let lint = self
            .client
            .raw_grpc()
            .lint_provider_profiles(proto::LintProviderProfilesRequest {
                workspace_scope: scope.clone(),
                profiles: vec![item.clone()],
            })
            .await
            .map_err(|status| anyhow!("LintProviderProfiles {source}: {}", status.message()))?
            .into_inner();
        if !lint.valid {
            bail!("{source} does not lint: {:?}", lint.diagnostics);
        }
        match resource_version {
            None => {
                let response = self
                    .client
                    .raw_grpc()
                    .import_provider_profiles(proto::ImportProviderProfilesRequest {
                        workspace_scope: scope,
                        profiles: vec![item],
                        request_id: String::new(),
                    })
                    .await
                    .map_err(|status| {
                        anyhow!("ImportProviderProfiles {source}: {}", status.message())
                    })?
                    .into_inner();
                if !response.imported {
                    bail!("{source} was not imported: {:?}", response.diagnostics);
                }
            }
            Some(version) => {
                let response = self
                    .client
                    .raw_grpc()
                    .update_provider_profiles(proto::UpdateProviderProfilesRequest {
                        workspace_scope: scope,
                        profile: Some(item),
                        expected_resource_version: version,
                        id: profile.id.clone(),
                        request_id: String::new(),
                    })
                    .await
                    .map_err(|status| {
                        anyhow!("UpdateProviderProfiles {source}: {}", status.message())
                    })?
                    .into_inner();
                if !response.updated {
                    bail!("{source} was not updated: {:?}", response.diagnostics);
                }
            }
        }
        Ok(())
    }

    /// Deletes the template if it exists and creates it.
    pub async fn replace_template(
        &self,
        workspace: &str,
        template: &proto::SandboxWorkloadTemplate,
        exists: bool,
    ) -> Result<()> {
        let name = template
            .metadata
            .as_ref()
            .map(|metadata| metadata.name.clone())
            .unwrap_or_default();
        let scope = Some(proto::workspace_selector(workspace));
        if exists {
            self.client
                .raw_grpc()
                .delete_sandbox_template(proto::DeleteSandboxTemplateRequest {
                    workspace_scope: scope.clone(),
                    name: name.clone(),
                    allow_missing: true,
                    request_id: String::new(),
                })
                .await
                .map_err(|status| {
                    anyhow!(
                        "DeleteSandboxTemplate {workspace}/{name}: {}",
                        status.message()
                    )
                })?;
        }
        self.client
            .raw_grpc()
            .create_sandbox_template(proto::CreateSandboxTemplateRequest {
                workspace_scope: scope,
                template: Some(template.clone()),
                request_id: String::new(),
            })
            .await
            .map_err(|status| {
                anyhow!(
                    "CreateSandboxTemplate {workspace}/{name}: {}",
                    status.message()
                )
            })?;
        Ok(())
    }

    /// Stops and deletes the sandbox if it exists, keeping its workspace, then creates it,
    /// exposes its services and waits until it is ready.
    pub async fn replace_sandbox(
        &self,
        workspace: &str,
        name: &str,
        request: &proto::CreateSandboxRequest,
        services: &[Service],
        exists: bool,
    ) -> Result<()> {
        let scope = Some(proto::workspace_selector(workspace));
        if exists {
            if self.phase(workspace, name).await? != Some(proto::SandboxPhase::Stopped) {
                self.client
                    .raw_grpc()
                    .stop_sandbox(proto::StopSandboxRequest {
                        workspace_scope: scope.clone(),
                        name: name.to_owned(),
                        request_id: String::new(),
                    })
                    .await
                    .map_err(|status| {
                        anyhow!("StopSandbox {workspace}/{name}: {}", status.message())
                    })?;
                self.await_phase(
                    workspace,
                    name,
                    Some(proto::SandboxPhase::Stopped),
                    STOP_TIMEOUT,
                )
                .await?;
            }
            self.client
                .raw_grpc()
                .delete_sandbox(proto::DeleteSandboxRequest {
                    workspace_scope: scope.clone(),
                    name: name.to_owned(),
                    allow_missing: true,
                    keep_workspace: true,
                    request_id: String::new(),
                })
                .await
                .map_err(|status| {
                    anyhow!("DeleteSandbox {workspace}/{name}: {}", status.message())
                })?;
            self.await_phase(workspace, name, None, STOP_TIMEOUT)
                .await?;
        }
        self.client
            .raw_grpc()
            .create_sandbox(request.clone())
            .await
            .map_err(|status| anyhow!("CreateSandbox {workspace}/{name}: {}", status.message()))?;
        self.await_phase(
            workspace,
            name,
            Some(proto::SandboxPhase::Ready),
            READY_TIMEOUT,
        )
        .await?;
        for service in services {
            self.ensure_service(workspace, name, service).await?;
        }
        Ok(())
    }

    /// Stops and starts a sandbox, keeping its specification, and waits until it is ready.
    pub async fn restart_sandbox(&self, workspace: &str, name: &str) -> Result<()> {
        let scope = Some(proto::workspace_selector(workspace));
        self.client
            .raw_grpc()
            .stop_sandbox(proto::StopSandboxRequest {
                workspace_scope: scope.clone(),
                name: name.to_owned(),
                request_id: String::new(),
            })
            .await
            .map_err(|status| anyhow!("StopSandbox {workspace}/{name}: {}", status.message()))?;
        self.await_phase(
            workspace,
            name,
            Some(proto::SandboxPhase::Stopped),
            STOP_TIMEOUT,
        )
        .await?;
        self.client
            .raw_grpc()
            .start_sandbox(proto::StartSandboxRequest {
                workspace_scope: scope,
                name: name.to_owned(),
                request_id: String::new(),
            })
            .await
            .map_err(|status| anyhow!("StartSandbox {workspace}/{name}: {}", status.message()))?;
        self.await_phase(
            workspace,
            name,
            Some(proto::SandboxPhase::Ready),
            READY_TIMEOUT,
        )
        .await
    }

    /// Exposes the service unless an exposure of its name already matches it.
    pub async fn ensure_service(
        &self,
        workspace: &str,
        sandbox: &str,
        service: &Service,
    ) -> Result<()> {
        let scope = Some(proto::workspace_selector(workspace));
        let listed = self
            .client
            .raw_grpc()
            .list_services(proto::ListServicesRequest {
                workspace_scope: scope.clone(),
                page_size: 100,
                page_token: String::new(),
                sandbox: sandbox.to_owned(),
            })
            .await
            .map_err(|status| anyhow!("ListServices {workspace}/{sandbox}: {}", status.message()))?
            .into_inner()
            .services;
        let declared = declared_service(service);
        let existing = listed
            .iter()
            .filter_map(|listed| listed.endpoint.as_ref())
            .map(active_service)
            .find(|active| active["name"] == declared["name"]);
        match existing {
            Some(active) if compare(&declared, &active).is_empty() => return Ok(()),
            Some(_) => {
                self.client
                    .raw_grpc()
                    .delete_service(proto::DeleteServiceRequest {
                        workspace_scope: scope.clone(),
                        name: service.name.clone(),
                        sandbox: sandbox.to_owned(),
                        allow_missing: true,
                        request_id: String::new(),
                    })
                    .await
                    .map_err(|status| {
                        anyhow!(
                            "DeleteService {workspace}/{sandbox}/{}: {}",
                            service.name,
                            status.message()
                        )
                    })?;
            }
            None => {}
        }
        let authorization_mode = match service.authorization.as_str() {
            "bearer-passthrough" => proto::ServiceAuthorizationMode::BearerPassthrough,
            "strip" => proto::ServiceAuthorizationMode::Strip,
            other => bail!(
                "service {} names the unknown authorization mode {other}",
                service.name
            ),
        };
        self.client
            .raw_grpc()
            .expose_service(proto::ExposeServiceRequest {
                workspace_scope: scope,
                name: service.name.clone(),
                target_port: service.port,
                domain: false,
                sandbox: sandbox.to_owned(),
                request_id: String::new(),
                authorization_mode: authorization_mode as i32,
            })
            .await
            .map_err(|status| {
                anyhow!(
                    "ExposeService {workspace}/{sandbox}/{}: {}",
                    service.name,
                    status.message()
                )
            })?;
        Ok(())
    }

    /// A file in a sandbox, read with `cat` through native exec.
    pub async fn read_file(&self, workspace: &str, sandbox: &str, path: &str) -> Result<String> {
        let output = self.exec(workspace, sandbox, &["/bin/cat", path]).await?;
        Ok(output)
    }

    /// The SHA-256 of each existing file of `paths` in a sandbox, by path, through native
    /// exec; no content leaves the sandbox.
    pub async fn digests(
        &self,
        workspace: &str,
        sandbox: &str,
        paths: &[String],
    ) -> Result<std::collections::BTreeMap<String, String>> {
        let mut command = vec![
            "/bin/sh",
            "-c",
            "for f; do if [ -f \"$f\" ]; then /usr/bin/sha256sum \"$f\"; fi; done",
            "sh",
        ];
        command.extend(paths.iter().map(String::as_str));
        let output = self.exec(workspace, sandbox, &command).await?;
        Ok(output
            .lines()
            .filter_map(|line| line.split_once("  "))
            .map(|(digest, path)| (path.to_owned(), digest.to_owned()))
            .collect())
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

/// The template as `sandbox template create` makes it, from the lock, the filled driver
/// configuration and its declared release note.
pub fn template_proto(
    inputs: &Inputs,
    template: &Template,
) -> Result<proto::SandboxWorkloadTemplate> {
    let image = template
        .image
        .as_deref()
        .map(|name| inputs.image(name))
        .transpose()?;
    let driver_config = template
        .driver_config
        .as_ref()
        .map(|file| driver_config(inputs, file))
        .transpose()?;
    let resources =
        (template.cpu.is_some() || template.memory.is_some()).then(|| proto::SandboxResources {
            cpu: template.cpu.clone().unwrap_or_default(),
            memory: template.memory.clone().unwrap_or_default(),
            gpu: None,
        });
    Ok(proto::SandboxWorkloadTemplate {
        metadata: Some(proto::ObjectMeta {
            name: template.name.clone(),
            workspace: template.workspace.clone(),
            annotations: template
                .release_note
                .iter()
                .map(|note| (RELEASE_NOTE.to_owned(), note.clone()))
                .collect(),
            ..Default::default()
        }),
        spec: Some(proto::SandboxWorkloadTemplateSpec {
            workload: Some(proto::SandboxWorkloadConfig {
                image: image.unwrap_or_default(),
                environment: template.environment.clone().into_iter().collect(),
                resources,
            }),
            driver_config,
            desired_service_level: None,
        }),
    })
}

/// The declared template against the one that runs, its release note compared as any
/// other member. The note describes a version, so a template that differs but declares
/// the note that runs is refused: a new version needs its own note. A template that
/// declares no note needs none.
fn compare_template(
    subject: String,
    workspace: &str,
    declared: proto::SandboxWorkloadTemplate,
    live: &proto::SandboxWorkloadTemplate,
) -> Item {
    let differences = compare(&active_template(&declared), &active_template(live));
    let note = release_note(&declared);
    if !differences.is_empty() && note.is_some() && note == release_note(live) {
        let members: Vec<String> = differences
            .iter()
            .map(|difference| difference.path.to_string())
            .collect();
        return Item::new(
            subject,
            State::Refused(format!(
                "it differs from what runs ({}) but declares the release note that runs: a new version needs its own note",
                members.join(", ")
            )),
        );
    }
    Item::compared(subject, differences, None).with_action(Action::ReplaceTemplate {
        workspace: workspace.to_owned(),
        template: Box::new(declared),
        exists: true,
    })
}

fn release_note(template: &proto::SandboxWorkloadTemplate) -> Option<&String> {
    template.metadata.as_ref()?.annotations.get(RELEASE_NOTE)
}

fn driver_config(inputs: &Inputs, file: &std::path::Path) -> Result<prost_types::Struct> {
    let value: Value = serde_json::from_str(&inputs.filled(file)?)
        .with_context(|| format!("{} is not JSON", file.display()))?;
    match json_to_protobuf(&value).kind {
        Some(prost_types::value::Kind::StructValue(config)) => Ok(config),
        _ => bail!("{} is not a JSON object", file.display()),
    }
}

pub fn active_template(template: &proto::SandboxWorkloadTemplate) -> Value {
    let metadata = template.metadata.clone().unwrap_or_default();
    let spec = template.spec.clone().unwrap_or_default();
    let workload = spec.workload.unwrap_or_default();
    let resources = workload.resources.unwrap_or_default();
    let mut value = json!({
        "image": workload.image,
        "environment": workload.environment.into_iter().collect::<BTreeMap<_, _>>(),
        "resources": {"cpu": resources.cpu, "memory": resources.memory},
        "driver_config": spec.driver_config.as_ref().map_or(Value::Null, struct_to_json),
        "labels": metadata.labels.into_iter().collect::<BTreeMap<_, _>>(),
        "annotations": metadata.annotations.into_iter().collect::<BTreeMap<_, _>>(),
    });
    if let Some(gpu) = resources.gpu.and_then(|gpu| gpu.count) {
        value["resources"]["gpu"] = json!(gpu);
    }
    value
}

/// The creation of a sandbox as `sandbox create --no-auto-providers --detach` makes it,
/// with the image from the lock, the canonical policy, the filled driver configuration
/// and the private environment values.
pub fn sandbox_request(inputs: &Inputs, sandbox: &Sandbox) -> Result<proto::CreateSandboxRequest> {
    let image = inputs.image(&sandbox.image)?;
    let policy = openshell_policy::parse_sandbox_policy(&inputs.filled(&sandbox.policy)?)
        .map_err(|error| anyhow!("{error}"))
        .and_then(|policy| {
            openshell_policy::validate_and_canonicalize_sandbox_policy(policy)
                .map_err(|error| anyhow!("{error}"))
        })
        .with_context(|| format!("{} is not a valid policy", sandbox.policy.display()))?;
    let driver_config = sandbox
        .driver_config
        .as_ref()
        .map(|file| driver_config(inputs, file))
        .transpose()?;
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
    let resources = match json_to_protobuf(&resource_limits(
        sandbox.cpu.as_deref(),
        sandbox.memory.as_deref(),
    ))
    .kind
    {
        Some(prost_types::value::Kind::StructValue(resources)) => Some(resources),
        _ => None,
    };
    Ok(proto::CreateSandboxRequest {
        workspace_scope: Some(proto::workspace_selector(&sandbox.workspace)),
        name: sandbox.name.clone(),
        spec: Some(proto::SandboxSpec {
            environment: environment.into_iter().collect(),
            template: Some(proto::SandboxTemplate {
                image,
                resources,
                driver_config,
                ..Default::default()
            }),
            policy: Some(policy),
            providers: sandbox.providers.clone(),
            command: sandbox.command.clone(),
            restart_policy: match sandbox.restart_policy {
                crate::declaration::RestartPolicy::Always => proto::SandboxRestartPolicy::Always,
                crate::declaration::RestartPolicy::OnFailure => {
                    proto::SandboxRestartPolicy::OnFailure
                }
                crate::declaration::RestartPolicy::Never => proto::SandboxRestartPolicy::Never,
            } as i32,
            resource_requirements: sandbox.gpu.map(|count| proto::ResourceRequirements {
                gpu: Some(proto::GpuResourceRequirements { count: Some(count) }),
            }),
            ..Default::default()
        }),
        ..Default::default()
    })
}

/// What a creation request makes, in the form `active_sandbox` reads.
pub fn requested_sandbox(request: &proto::CreateSandboxRequest) -> Result<Value> {
    active_sandbox(&proto::Sandbox {
        spec: request.spec.clone(),
        ..Default::default()
    })
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

fn service_subject(sandbox: &Sandbox, service: &Service) -> String {
    format!(
        "service {}/{}:{}",
        sandbox.key(),
        service.name,
        service.port
    )
}

fn expose(sandbox: &Sandbox, service: &Service, exists: bool) -> Action {
    Action::ExposeService {
        workspace: sandbox.workspace.clone(),
        sandbox: sandbox.name.clone(),
        service: service.clone(),
        exists,
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

/// JSON as a protobuf `Value`, as `--driver-config-json` sends it.
pub fn json_to_protobuf(value: &Value) -> prost_types::Value {
    use prost_types::value::Kind;
    let kind = match value {
        Value::Null => Kind::NullValue(0),
        Value::Bool(value) => Kind::BoolValue(*value),
        Value::Number(number) => Kind::NumberValue(number.as_f64().unwrap_or_default()),
        Value::String(text) => Kind::StringValue(text.clone()),
        Value::Array(items) => Kind::ListValue(prost_types::ListValue {
            values: items.iter().map(json_to_protobuf).collect(),
        }),
        Value::Object(members) => Kind::StructValue(prost_types::Struct {
            fields: members
                .iter()
                .map(|(name, member)| (name.clone(), json_to_protobuf(member)))
                .collect(),
        }),
    };
    prost_types::Value { kind: Some(kind) }
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
        let Some(prost_types::value::Kind::StructValue(as_struct)) =
            json_to_protobuf(&declared).kind
        else {
            panic!("an object is a Struct");
        };
        assert_eq!(struct_to_json(&as_struct), declared);
    }

    #[test]
    fn a_sandbox_reads_back_as_its_creation_inputs() {
        // A recorded Media sandbox as GetSandbox answers it: providers in attachment
        // order, the CLI's --cpu and --memory as template limits.
        let mut limits = prost_types::Struct::default();
        limits.fields.insert(
            "limits".to_owned(),
            json_to_protobuf(&json!({"cpu": "1", "memory": "1Gi"})),
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

    /// computer-v1's release note as the reference installation's gateway answers it.
    const NOTE: &str = "The Hub CLI now works inside your Computer, as you.\nRun veoveo-hub-sdk-cli tools or call: it uses your Hub sign-in, like OpenCode. login, logout and ask are not needed here.";

    /// A template as `template_proto` declares it: computer-v1's size and environment,
    /// with `image`, a Lore mount of `lore` and `note`.
    fn declared_template(
        name: &str,
        image: &str,
        lore: &str,
        note: Option<&str>,
    ) -> proto::SandboxWorkloadTemplate {
        let Some(prost_types::value::Kind::StructValue(driver_config)) = json_to_protobuf(
            &json!({"kubernetes": {"volumes": [{"name": "lore", "image": {"reference": lore}}]}}),
        )
        .kind
        else {
            panic!("an object is a Struct");
        };
        proto::SandboxWorkloadTemplate {
            metadata: Some(proto::ObjectMeta {
                name: name.into(),
                workspace: "installation".into(),
                annotations: note
                    .map(|note| (RELEASE_NOTE.to_owned(), note.to_owned()))
                    .into_iter()
                    .collect(),
                ..Default::default()
            }),
            spec: Some(proto::SandboxWorkloadTemplateSpec {
                workload: Some(proto::SandboxWorkloadConfig {
                    image: image.into(),
                    environment: [("SSL_CERT_DIR".to_owned(), "/etc/ssl/certs".to_owned())]
                        .into_iter()
                        .collect(),
                    resources: Some(proto::SandboxResources {
                        cpu: "2".into(),
                        memory: "4Gi".into(),
                        gpu: None,
                    }),
                }),
                driver_config: Some(driver_config),
                desired_service_level: None,
            }),
        }
    }

    /// The template as GetSandboxTemplate answers it once created: with the gateway's
    /// object ID, resource version and creation time.
    fn running_template(
        name: &str,
        image: &str,
        lore: &str,
        note: Option<&str>,
    ) -> proto::SandboxWorkloadTemplate {
        let mut template = declared_template(name, image, lore, note);
        let metadata = template.metadata.as_mut().unwrap();
        metadata.id = "3f1c2a9e-5d47-4f0b-9a61-2c8e7b4d1f05".into();
        metadata.resource_version = 1;
        metadata.created_time = Some(prost_types::Timestamp {
            seconds: 1_792_886_400,
            nanos: 0,
        });
        template
    }

    const COMPUTER: &str = "registry/computer@sha256:1";
    const LORE: &str = "registry/lore@sha256:1";

    /// computer-v1 as it runs: the lock's images and Bioma's note.
    fn running_computer() -> proto::SandboxWorkloadTemplate {
        running_template("computer-v1", COMPUTER, LORE, Some(NOTE))
    }

    fn compare_templates(
        declared: proto::SandboxWorkloadTemplate,
        live: &proto::SandboxWorkloadTemplate,
    ) -> Item {
        compare_template("template".into(), "installation", declared, live)
    }

    fn difference_paths(item: &Item) -> Vec<String> {
        match &item.state {
            State::Differs { differences, .. } => differences
                .iter()
                .map(|difference| difference.path.to_string())
                .collect(),
            other => panic!("expected a difference, got {other:?}"),
        }
    }

    #[test]
    fn a_template_reads_back_as_declared_with_its_release_note() {
        let item = compare_templates(
            declared_template("computer-v1", COMPUTER, LORE, Some(NOTE)),
            &running_computer(),
        );
        assert!(matches!(item.state, State::Same), "{:?}", item.state);
        assert!(item.action.is_none());
    }

    #[test]
    fn a_new_version_that_keeps_the_running_release_note_is_refused() {
        // A Lore-only upgrade: the lock's Lore digest changes and nothing else.
        let declared = declared_template(
            "computer-v1",
            COMPUTER,
            "registry/lore@sha256:2",
            Some(NOTE),
        );
        let item = compare_templates(declared, &running_computer());
        let State::Refused(reason) = &item.state else {
            panic!("expected a refusal, got {:?}", item.state);
        };
        assert_eq!(
            reason,
            "it differs from what runs (driver_config.kubernetes.volumes[name=lore].image.reference) but declares the release note that runs: a new version needs its own note"
        );
        assert!(item.action.is_none());
        assert!(item.blocker().is_some());
    }

    #[test]
    fn a_new_version_publishes_its_declared_release_note() {
        let declared = declared_template(
            "computer-v1",
            COMPUTER,
            "registry/lore@sha256:2",
            Some("Lore 1.5 in your Computer."),
        );
        let item = compare_templates(declared, &running_computer());
        assert_eq!(
            difference_paths(&item),
            [
                r#"annotations["veoveo.ai/release-note"]"#,
                "driver_config.kubernetes.volumes[name=lore].image.reference"
            ]
        );
        let Some(Action::ReplaceTemplate {
            template,
            exists: true,
            ..
        }) = &item.action
        else {
            panic!("expected a replacement, got {:?}", item.action);
        };
        assert_eq!(
            release_note(template).map(String::as_str),
            Some("Lore 1.5 in your Computer.")
        );
    }

    #[test]
    fn a_changed_release_note_alone_is_a_difference() {
        let declared = declared_template("computer-v1", COMPUTER, LORE, Some("Clearer words."));
        let item = compare_templates(declared, &running_computer());
        assert_eq!(
            difference_paths(&item),
            [r#"annotations["veoveo.ai/release-note"]"#]
        );
        assert!(item.action.is_some());
    }

    #[test]
    fn a_template_that_declares_no_release_note_needs_none() {
        // factory-v1 carries no note, and Hub reads none from it.
        let live = running_template("factory-v1", "", LORE, None);
        let declared = declared_template("factory-v1", "", "registry/lore@sha256:2", None);
        let item = compare_templates(declared, &live);
        assert_eq!(
            difference_paths(&item),
            ["driver_config.kubernetes.volumes[name=lore].image.reference"]
        );
        assert!(item.action.is_some());
        // Nor does a template that stops declaring one.
        let declared = declared_template("computer-v1", COMPUTER, "registry/lore@sha256:2", None);
        let item = compare_templates(declared, &running_computer());
        assert_eq!(
            difference_paths(&item),
            [
                r#"annotations["veoveo.ai/release-note"]"#,
                "driver_config.kubernetes.volumes[name=lore].image.reference"
            ]
        );
        assert!(item.action.is_some());
    }
}
