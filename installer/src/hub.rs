//! Hub's control plane, read and published through Hub's administrative route as the
//! operator, an installation administrator signed in through the browser with the Hub
//! SDK's sign-in. This repository's seed, filled with the handbook and the names of
//! Hub's trusted files, is what the installation publishes; Hub's validation answers it
//! as Hub would keep and serve it, with its SHA-256, which is compared with the active
//! revision's, so Hub's own serialization, with its defaults, decides what differs.
//! The members people change (their grants, memberships, Work Context output owners and
//! initial grants, repositories and assignments) are never the installation's: they are
//! left out of the comparison, and a publication keeps them as they are at that moment.
//! Nothing resets the control plane; a first installation's Hub seeds itself.
//!
//! Hub's trusted files that come from this repository, such as the policies Hub hands
//! to Computers and factory instances, are named by their content and read in Hub's
//! sandbox through native exec.

use std::{
    collections::BTreeSet,
    path::Path,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow, bail};
use serde_json::Value;
use sha2::{Digest as _, Sha256};
use url::Url;
use veoveo_hub_sdk::{HubProfileId, OAuthClientId, client::BearerSource as _};
use veoveo_hub_sdk_cli::{
    grant::{GrantBearer, GrantStore},
    hub::{HubAccess, HubAuthentication},
    sign_in::{SignIn, sign_in},
};
use veoveo_types::ScopeName;

use crate::{
    action::Action,
    declaration::{Hub, HubClaim, Sandbox},
    difference::{Difference, Path as ValuePath, compare, remove},
    inputs::Inputs,
    openshell::Gateway,
    plan::{Item, State},
};

/// The public client the installation registers for a terminal's browser sign-in.
pub const SIGN_IN_CLIENT: &str = "veoveo-hub-sdk-cli";
pub const SIGN_IN_REDIRECT: &str = "http://127.0.0.1:3211/callback";
pub const SIGN_IN_SCOPE: &str = "mcp";

/// Members of the control plane that people change, never the installation.
pub const PEOPLE: [&str; 6] = [
    "authorization.grants",
    "workContexts.*.memberships",
    "workContexts.*.outputPolicy.owner",
    "workContexts.*.outputPolicy.initialGrants",
    "lore.repositories",
    "lore.assignments",
];

/// Sections of the control plane Hub reads only when it starts: a change to one takes
/// effect once Hub restarts.
pub const READ_AT_START: [&str; 1] = ["factory"];

/// How long Hub may take to report a server it just started healthy.
const REGISTRATION_TIMEOUT: Duration = Duration::from_secs(180);

// ----- Hub's trusted claim -----

/// The name of a claim file by its content: `<name>-<12 hexadecimal digits>.yaml`.
pub fn claim_file_name(name: &str, content: &str) -> String {
    let digest = Sha256::digest(content.as_bytes());
    let hex: String = digest
        .iter()
        .take(6)
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("{name}-{hex}.yaml")
}

/// The claim's declared files, as `(file name, content)`, in declaration order.
pub fn claim_files(inputs: &Inputs, claim: &HubClaim) -> Result<Vec<(String, String, String)>> {
    claim
        .files
        .iter()
        .map(|file| {
            let content = inputs.filled(&file.source)?;
            Ok((
                file.name.clone(),
                claim_file_name(&file.name, &content),
                content,
            ))
        })
        .collect()
}

/// Replaces each `"@claim:<name>@"` with the path of the claim file of that name.
pub fn fill_claim(
    text: &str,
    directory: &str,
    files: &[(String, String, String)],
) -> Result<String> {
    const OPEN: &str = "@claim:";
    let mut filled = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(OPEN) {
        filled.push_str(&rest[..start]);
        let after = &rest[start + OPEN.len()..];
        let end = after
            .find('@')
            .ok_or_else(|| anyhow!("a @claim: placeholder is not closed"))?;
        let name = &after[..end];
        let (_, file, _) = files
            .iter()
            .find(|(declared, _, _)| declared == name)
            .ok_or_else(|| anyhow!("the claim declares no file {name}"))?;
        filled.push_str(&format!("{directory}/{file}"));
        rest = &after[end + 1..];
    }
    filled.push_str(rest);
    Ok(filled)
}

/// The claim's declared files, as `claim_files` names them, provided its seed Job is
/// declared too: apply writes the files with it and removes files with a Job made from it.
/// A claim whose files or manifest cannot be filled cannot be written, and one whose
/// manifest still holds a render marker is not ready.
pub fn declared_claim(inputs: &Inputs, claim: &HubClaim) -> Result<Vec<(String, String, String)>> {
    let files = claim_files(inputs, claim)?;
    crate::kubernetes::seed_job(inputs, claim)?;
    Ok(files)
}

pub async fn plan_claim(inputs: &Inputs, claim: &HubClaim, gateway: &Gateway) -> Vec<Item> {
    let subject = format!("Hub's trusted files in {}", claim.directory);
    let files = match declared_claim(inputs, claim) {
        Ok(files) => files,
        Err(error) => return vec![Item::unusable(subject, &error)],
    };
    let present = match gateway
        .sandbox_exists(&claim.workspace, &claim.sandbox)
        .await
    {
        Ok(false) => None,
        Ok(true) => match gateway
            .list_directory(&claim.workspace, &claim.sandbox, &claim.directory)
            .await
        {
            Ok(names) => Some(names),
            Err(error) => return vec![Item::unread(subject, format!("{error:#}"))],
        },
        Err(error) => return vec![Item::unread(subject, format!("{error:#}"))],
    };
    let mut differences = Vec::new();
    let mut writes = Vec::new();
    for (_, file, content) in &files {
        let active = match &present {
            Some(names) if names.contains(file) => {
                let path = format!("{}/{file}", claim.directory);
                match gateway
                    .read_file(&claim.workspace, &claim.sandbox, &path)
                    .await
                {
                    Ok(active) => Some(active),
                    Err(error) => return vec![Item::unread(subject, format!("{error:#}"))],
                }
            }
            _ => None,
        };
        if active.as_deref() != Some(content.as_str()) {
            differences.push(Difference::new(
                ValuePath::root().member(file),
                Some(Value::String(format!("{} bytes", content.len()))),
                active.map(|active| Value::String(format!("{} bytes", active.len()))),
            ));
            writes.push((file.clone(), content.clone()));
        }
    }
    let mut items = Vec::new();
    if !claim.private.is_empty() {
        match private_files(inputs, claim, gateway, present.is_some()).await {
            Ok((private_differences, private_writes, refused)) => {
                differences.extend(private_differences);
                writes.extend(private_writes);
                items.extend(refused);
            }
            Err(error) => items.push(Item::unread(
                format!("Hub's private trusted files in {}", claim.directory),
                format!("{error:#}"),
            )),
        }
    }
    let note = present.is_none().then(|| {
        format!(
            "{}/{} does not run, so its claim was not read",
            claim.workspace, claim.sandbox
        )
    });
    let item = Item::compared(subject, differences, note).with_action(Action::WriteClaimFiles {
        claim: claim.clone(),
        files: writes,
    });
    items.insert(0, item);
    items
}

/// The claim's private files against the private directory's, which `identity hub`
/// prepares, by SHA-256, read in Hub's sandbox; no content leaves either side. Hub
/// reads them when it starts, so a missing one is written, and so is a changed seed,
/// which Hub reads only for an empty control plane; a changed key or key set is a
/// reviewed rotation, which apply does not make.
async fn private_files(
    inputs: &Inputs,
    claim: &HubClaim,
    gateway: &Gateway,
    running: bool,
) -> Result<(Vec<Difference>, Vec<(String, String)>, Vec<Item>)> {
    let (directory, _) = crate::identity::hub::directory(inputs)?;
    let mut local = Vec::new();
    for name in &claim.private {
        let path = directory.join(name);
        let text = std::fs::read_to_string(&path).with_context(|| {
            format!(
                "{} is not there; prepare Hub's trusted files with `veoveo-installer identity hub`",
                path.display()
            )
        })?;
        local.push((name.clone(), text));
    }
    let active = if running {
        let paths: Vec<String> = claim
            .private
            .iter()
            .map(|name| format!("{}/{name}", claim.directory))
            .collect();
        gateway
            .digests(&claim.workspace, &claim.sandbox, &paths)
            .await?
    } else {
        Default::default()
    };
    let (mut differences, mut writes, mut refused) = (Vec::new(), Vec::new(), Vec::new());
    for (name, text) in local {
        let digest = sha256_hex(&text);
        let held = active.get(&format!("{}/{name}", claim.directory));
        if held == Some(&digest) {
            continue;
        }
        if held.is_some() && name != crate::identity::hub::SEED_FILE {
            refused.push(Item::new(
                format!("Hub's private trusted file {}/{name}", claim.directory),
                State::Refused("it differs from the private directory's; Hub reads it when it starts, and replacing it is a reviewed rotation".to_owned()),
            ));
            continue;
        }
        differences.push(Difference::new(
            ValuePath::root().member(&name),
            Some(Value::String("the private directory's".to_owned())),
            held.map(|_| Value::String("another".to_owned())),
        ));
        writes.push((name, text));
    }
    Ok((differences, writes, refused))
}

fn sha256_hex(text: &str) -> String {
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

// ----- the seed -----

/// The seed documents merged by their typed keys, with the handbook and the claim's
/// file names filled.
pub fn declared_seed(inputs: &Inputs, hub: &Hub) -> Result<Value> {
    let claim = inputs
        .declaration
        .claim(&hub.claim)
        .with_context(|| format!("installation.toml declares no claim step {}", hub.claim))?;
    let files = claim_files(inputs, claim)?;
    let mut seed = Value::Object(serde_json::Map::new());
    for file in &hub.seed {
        let text = fill_claim(&inputs.text(file)?, &claim.directory, &files)
            .with_context(|| format!("{} cannot be filled", file.display()))?;
        let document: Value = serde_json::from_str(&text)
            .with_context(|| format!("{} is not JSON", file.display()))?;
        merge_seed(&mut seed, document);
    }
    let handbook = inputs.text(&hub.handbook)?;
    seed.get_mut("computers")
        .and_then(Value::as_object_mut)
        .context("the seed has no computers section for the handbook")?
        .insert("handbook".to_owned(), Value::String(handbook));
    Ok(seed)
}

/// Merges one seed document into the others by their typed keys: sections merge member
/// by member, an entry of a list joins it or, when an entry of its `id` is there, merges
/// into that entry, and any other value of the later document wins. No `id` appears
/// twice.
pub fn merge_seed(base: &mut Value, layer: Value) {
    match (base, layer) {
        (Value::Object(base), Value::Object(layer)) => {
            for (name, value) in layer {
                match base.get_mut(&name) {
                    Some(existing) => merge_seed(existing, value),
                    None => {
                        base.insert(name, value);
                    }
                }
            }
        }
        (Value::Array(base), Value::Array(layer)) if identified(base) && identified(&layer) => {
            for item in layer {
                match base
                    .iter_mut()
                    .find(|existing| existing["id"] == item["id"])
                {
                    Some(existing) => merge_seed(existing, item),
                    None => base.push(item),
                }
            }
        }
        (base, layer) => *base = layer,
    }
}

fn identified(items: &[Value]) -> bool {
    items.iter().all(|item| item["id"].is_string())
}

/// Hub's public origin: the origin of the profile's protected resource.
pub fn origin(seed: &Value, profile: &str) -> Result<Url> {
    let resource = seed["profiles"]
        .as_array()
        .and_then(|profiles| {
            profiles
                .iter()
                .find(|item| item["id"].as_str() == Some(profile))
        })
        .and_then(|item| item["protectedResource"].as_str())
        .with_context(|| format!("the seed has no profile {profile} with a protected resource"))?;
    let resource = Url::parse(resource).with_context(|| format!("{resource:?} is not a URL"))?;
    Url::parse(&resource.origin().ascii_serialization())
        .context("the protected resource has no origin")
}

/// The control plane without the members people change.
pub fn installation_owned(mut control_plane: Value) -> Value {
    for path in PEOPLE {
        remove(&mut control_plane, path);
    }
    control_plane
}

/// The document to publish: the declared members, with the members people change taken
/// from the active revision. A Work Context the active revision lacks gets no members,
/// and no output owner or initial grants: the seed's example people are never published.
pub fn publication(declared: &Value, active: &Value) -> Value {
    let mut document = installation_owned(declared.clone());
    if let (Some(grants), Some(authorization)) = (
        active.pointer("/authorization/grants"),
        document
            .get_mut("authorization")
            .and_then(Value::as_object_mut),
    ) {
        authorization.insert("grants".to_owned(), grants.clone());
    }
    for member in ["repositories", "assignments"] {
        if let (Some(value), Some(lore)) = (
            active.pointer(&format!("/lore/{member}")),
            document.get_mut("lore").and_then(Value::as_object_mut),
        ) {
            lore.insert(member.to_owned(), value.clone());
        }
    }
    if let Some(contexts) = document
        .get_mut("workContexts")
        .and_then(Value::as_array_mut)
    {
        for context in contexts {
            let active_context = active["workContexts"]
                .as_array()
                .and_then(|active| active.iter().find(|other| other["id"] == context["id"]));
            let memberships = active_context
                .and_then(|other| other.get("memberships").cloned())
                .unwrap_or_else(|| Value::Array(Vec::new()));
            let Some(context) = context.as_object_mut() else {
                continue;
            };
            context.insert("memberships".to_owned(), memberships);
            for member in OUTPUT_PEOPLE {
                let Some(value) = active_context
                    .and_then(|other| other.pointer(&format!("/outputPolicy/{member}")))
                else {
                    continue;
                };
                if let Some(policy) = context
                    .entry("outputPolicy")
                    .or_insert_with(|| Value::Object(serde_json::Map::new()))
                    .as_object_mut()
                {
                    policy.insert(member.to_owned(), value.clone());
                }
            }
        }
    }
    document
}

/// The members of a Work Context's output policy that are people's.
const OUTPUT_PEOPLE: [&str; 2] = ["owner", "initialGrants"];

/// The Work Contexts of `document` whose output policy names no owner: a context the
/// active revision lacks, whose owner is a person's and set in Hub, never this
/// repository's example.
pub fn unowned_contexts(document: &Value) -> Vec<String> {
    document["workContexts"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|context| {
            context
                .get("outputPolicy")
                .is_some_and(|policy| policy.get("owner").is_none())
        })
        .map(|context| context["id"].as_str().unwrap_or_default().to_owned())
        .collect()
}

// ----- signing in -----

fn grant_store(inputs: &Inputs) -> Result<GrantStore> {
    let directory = match &inputs.private.hub.grants {
        Some(directory) => inputs.private.path(directory),
        None => GrantStore::default_directory(
            std::env::var_os("XDG_CONFIG_HOME"),
            std::env::var_os("HOME"),
        )?,
    };
    Ok(GrantStore::new(directory))
}

fn access(origin: &Url, profile: &str, grants: &GrantStore) -> Result<HubAccess> {
    Ok(HubAccess {
        hub: origin.clone(),
        connect: None,
        authentication: HubAuthentication::StoredGrant {
            grants: grants.clone(),
        },
        profile: HubProfileId::parse(profile)?,
    })
}

/// Signs the operator in to Hub through the browser and stores the grant where the Hub
/// SDK CLI keeps it.
pub async fn login(inputs: &Inputs, no_browser: bool) -> Result<()> {
    let hub = hub_step(inputs)?;
    let seed = declared_seed(inputs, hub)?;
    let origin = origin(&seed, &hub.profile)?;
    let grants = grant_store(inputs)?;
    let access = access(&origin, &hub.profile, &grants)?;
    let request = SignIn {
        client: OAuthClientId::parse(SIGN_IN_CLIENT)?,
        redirect_uri: Url::parse(SIGN_IN_REDIRECT)?,
        scope: ScopeName::parse(SIGN_IN_SCOPE)?,
        timeout: Duration::from_secs(300),
    };
    let signed_in = sign_in(&access, &grants, &request, |address| {
        eprintln!("Sign in to {origin} as an installation administrator:\n\n  {address}\n");
        if !no_browser {
            open_browser(address);
        }
    })
    .await?;
    println!(
        "Signed in to {} profile {}. The grant is in {}.",
        origin.origin().ascii_serialization(),
        hub.profile,
        signed_in.path.display()
    );
    Ok(())
}

fn open_browser(address: &Url) {
    let program = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(program)
        .arg(address.as_str())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

fn hub_step(inputs: &Inputs) -> Result<&Hub> {
    inputs
        .declaration
        .steps
        .iter()
        .find_map(|step| match &step.kind {
            crate::declaration::StepKind::Hub(hub) => Some(hub),
            _ => None,
        })
        .context("installation.toml declares no Hub step")
}

/// The operator's signed-in access to Hub's administrative routes.
pub struct Administrator {
    origin: Url,
    profile: String,
    http: reqwest::Client,
    bearer: GrantBearer,
}

impl Administrator {
    /// The stored sign-in; fails when there is none.
    pub fn signed_in(inputs: &Inputs, origin: &Url, profile: &str) -> Result<Self> {
        let grants = grant_store(inputs)?;
        let access = access(origin, profile, &grants)?;
        let profile_id = HubProfileId::parse(profile)?;
        let http = access.oauth_http()?;
        if grants.load(origin, &profile_id).ok().flatten().is_none() {
            bail!(
                "there is no stored Hub sign-in to {} for profile {profile} in {}; sign in as an installation administrator with `veoveo-installer login`",
                origin.origin().ascii_serialization(),
                grants.directory().display()
            );
        }
        let bearer = GrantBearer::load(grants, origin, &profile_id, http.clone()).map_err(|error| {
            let reason = error.to_string();
            let reason = reason.split("; ").next().unwrap_or_default().to_owned();
            anyhow!("the stored sign-in cannot be used: {reason}; sign in again with `veoveo-installer login`")
        })?;
        Ok(Self {
            origin: origin.clone(),
            profile: profile.to_owned(),
            http,
            bearer,
        })
    }

    fn address(&self, route: &str) -> Result<Url> {
        let mut address = self.origin.clone();
        address
            .path_segments_mut()
            .map_err(|()| anyhow!("{} cannot hold a path", self.origin))?
            .pop_if_empty()
            .extend(["admin", self.profile.as_str()])
            .extend(route.split('/'));
        Ok(address)
    }

    /// One administrative request; its answer must be 200 with JSON.
    async fn request(
        &self,
        method: reqwest::Method,
        route: &str,
        body: Option<&Value>,
    ) -> Result<Value> {
        let response = self.send(method.clone(), route, body, None).await?;
        answer(response, &method).await
    }

    /// One administrative request, renewing the sign-in once when Hub does not admit it.
    async fn send(
        &self,
        method: reqwest::Method,
        route: &str,
        body: Option<&Value>,
        if_match: Option<&str>,
    ) -> Result<reqwest::Response> {
        let address = self.address(route)?;
        for attempt in 0..2 {
            let token = self
                .bearer
                .caller_bearer()
                .await
                .map_err(|error| anyhow!("the sign-in could not be renewed: {error}"))?;
            let mut request = self
                .http
                .request(method.clone(), address.clone())
                .bearer_auth(token.expose())
                .timeout(Duration::from_secs(60));
            if let Some(body) = body {
                request = request.json(body);
            }
            if let Some(revision) = if_match {
                request = request.header(reqwest::header::IF_MATCH, format!("\"{revision}\""));
            }
            let response = request
                .send()
                .await
                .with_context(|| format!("cannot reach {address}"))?;
            if response.status() == reqwest::StatusCode::UNAUTHORIZED && attempt == 0 {
                self.bearer.forget_caller();
                continue;
            }
            return Ok(response);
        }
        unreachable!("the second attempt returns")
    }

    pub async fn control_plane(&self) -> Result<ActiveControlPlane> {
        let body = self
            .request(reqwest::Method::GET, "control-plane", None)
            .await?;
        let control_plane = body
            .get("controlPlane")
            .cloned()
            .context("Hub's answer holds no controlPlane")?;
        Ok(ActiveControlPlane {
            revision: body["revisionId"].as_str().map(str::to_owned),
            sha256: body["sha256"]
                .as_str()
                .context("Hub's answer holds no sha256")?
                .to_owned(),
            control_plane,
        })
    }

    /// What publishing `document` would make active, as Hub validates and prepares it
    /// without publishing: the document as Hub would keep and serve it with its SHA-256,
    /// or the reason Hub refuses it.
    pub async fn validate(&self, document: &Value) -> Result<Validation> {
        let method = reqwest::Method::POST;
        let response = self
            .send(
                method.clone(),
                "control-plane/validate",
                Some(document),
                None,
            )
            .await?;
        match response.status().as_u16() {
            400 => {
                let text = response.text().await.unwrap_or_default();
                Ok(Validation::Refused(refusal_reason(&text)))
            }
            404 | 405 => bail!(
                "Hub at {} does not validate control-plane documents (HTTP {}): it needs Hub with POST /admin/{{profile}}/control-plane/validate",
                self.origin,
                response.status().as_u16()
            ),
            _ => {
                let body = answer(response, &method).await?;
                Ok(Validation::Valid {
                    sha256: body["sha256"]
                        .as_str()
                        .context("Hub's validation holds no sha256")?
                        .to_owned(),
                    control_plane: body
                        .get("controlPlane")
                        .cloned()
                        .context("Hub's validation holds no controlPlane")?,
                })
            }
        }
    }

    /// Publishes a complete control plane as the successor of `based_on`, the revision
    /// it was made from. Hub validates it and keeps its active revision when it refuses
    /// it; it answers 412 when another revision became active meanwhile.
    pub async fn publish(&self, document: &Value, based_on: &str) -> Result<Publication> {
        let method = reqwest::Method::PUT;
        let response = self
            .send(
                method.clone(),
                "control-plane",
                Some(document),
                Some(based_on),
            )
            .await?;
        if response.status() == reqwest::StatusCode::PRECONDITION_FAILED {
            return Ok(Publication::Moved);
        }
        let body = answer(response, &method).await?;
        Ok(Publication::Active(
            body["revisionId"].as_str().unwrap_or_default().to_owned(),
        ))
    }

    pub async fn server_health(&self) -> Result<Value> {
        self.request(reqwest::Method::GET, "server-health", None)
            .await
    }
}

/// What a publication did.
#[derive(Debug, PartialEq, Eq)]
pub enum Publication {
    /// The revision it made is active.
    Active(String),
    /// Another revision became active since the one it was based on; nothing changed.
    Moved,
}

/// A 200 answer's JSON, or the reason Hub gave.
async fn answer(response: reqwest::Response, method: &reqwest::Method) -> Result<Value> {
    let address = response.url().clone();
    match response.status().as_u16() {
        200 => response.json().await.context("Hub answered no JSON"),
        401 => bail!(
            "Hub does not admit the stored sign-in (HTTP 401); sign in again with `veoveo-installer login`"
        ),
        403 => bail!(
            "Hub refused the signed-in person (HTTP 403): this needs an installation administrator"
        ),
        code => {
            let text = response.text().await.unwrap_or_default();
            bail!(
                "Hub answered HTTP {code} for {method} {address}: {}",
                text.trim()
            )
        }
    }
}

/// The reason of an RFC 9457 problem Hub answered: its detail, else its title, else the
/// text.
fn refusal_reason(text: &str) -> String {
    let problem: Value = serde_json::from_str(text).unwrap_or(Value::Null);
    problem["detail"]
        .as_str()
        .or_else(|| problem["title"].as_str())
        .unwrap_or(text.trim())
        .to_owned()
}

/// What Hub's administrative route answers: the active revision, when Hub serves it,
/// and the control plane as Hub serves it, with its SHA-256.
#[derive(Debug, Clone)]
pub struct ActiveControlPlane {
    pub revision: Option<String>,
    pub sha256: String,
    pub control_plane: Value,
}

/// What Hub answers for a document it validated without publishing it.
#[derive(Debug, Clone, PartialEq)]
pub enum Validation {
    /// The document as Hub would keep and serve it, with its SHA-256.
    Valid {
        sha256: String,
        control_plane: Value,
    },
    /// Hub refuses the document, for this reason.
    Refused(String),
}

/// What publishing the declared members over the active revision would change.
#[derive(Debug)]
pub(crate) struct Comparison {
    pub active: ActiveControlPlane,
    /// The declared members over the active revision's people: what would be published.
    pub document: Value,
    pub outcome: Outcome,
}

#[derive(Debug, PartialEq)]
pub(crate) enum Outcome {
    /// Hub refuses the document, for this reason.
    Refused(String),
    /// Where Hub's form of the document differs from the active revision's; none when
    /// their SHA-256 agree.
    Differs(Vec<Difference>),
}

/// Reads the active revision, makes the publication from it and has Hub validate it;
/// the two are the same when their SHA-256 agree. Where they differ is found between
/// Hub's two forms, never between this repository's text and Hub's.
pub(crate) async fn compare_with_hub(
    route: &impl ControlPlaneRoute,
    declared: &Value,
) -> Result<Comparison> {
    let active = route.read().await?;
    let document = publication(declared, &active.control_plane);
    let unowned = unowned_contexts(&document);
    let validation = if unowned.is_empty() {
        route.validate(&document).await?
    } else {
        Validation::Refused(format!(
            "the active revision has no Work Context {}, whose output owner is a person's and set in Hub; this repository's example owner is never published",
            unowned.join(", ")
        ))
    };
    let outcome = match validation {
        Validation::Refused(reason) => Outcome::Refused(reason),
        Validation::Valid { sha256, .. } if sha256 == active.sha256 => Outcome::Differs(Vec::new()),
        Validation::Valid {
            sha256,
            control_plane,
        } => {
            let mut differences = compare(
                &installation_owned(control_plane),
                &installation_owned(active.control_plane.clone()),
            );
            if differences.is_empty() {
                // They differ only in members one of them holds empty.
                differences.push(Difference::new(
                    ValuePath::root().member("sha256"),
                    Some(Value::String(sha256)),
                    Some(Value::String(active.sha256.clone())),
                ));
            }
            Outcome::Differs(differences)
        }
    };
    Ok(Comparison {
        active,
        document,
        outcome,
    })
}

// ----- the control plane step -----

pub async fn plan(inputs: &Inputs, hub: &Hub, gateway: Option<&Gateway>) -> Vec<Item> {
    let seed = match declared_seed(inputs, hub) {
        Ok(seed) => seed,
        Err(error) => return vec![Item::unusable("control plane", &error)],
    };
    let origin = match origin(&seed, &hub.profile) {
        Ok(origin) => origin,
        Err(error) => return vec![Item::unusable("control plane", &error)],
    };
    let subject = format!("control plane at {origin}");
    let Some(gateway) = gateway else {
        return vec![Item::unread(
            subject,
            "the OpenShell gateway was not reached, so Hub's sandbox was not read",
        )];
    };
    match gateway.sandbox_exists(&hub.workspace, &hub.sandbox).await {
        Ok(true) => {}
        Ok(false) => {
            return vec![Item::new(
                subject,
                State::NotCompared(format!(
                    "{}/{} does not run: a first installation's Hub seeds its control plane from its claim when it starts",
                    hub.workspace, hub.sandbox
                )),
            )];
        }
        Err(error) => return vec![Item::unread(subject, format!("{error:#}"))],
    }
    let administrator = match Administrator::signed_in(inputs, &origin, &hub.profile) {
        Ok(administrator) => administrator,
        Err(error) => return vec![Item::unread(subject, format!("{error:#}"))],
    };
    let claim = inputs.declaration.claim(&hub.claim);
    // Hub reads the trusted files a document names when it validates it, and the claim
    // step writes those this repository adds before this step runs.
    let unwritten = match claim {
        Some(claim) => match unwritten_files(gateway, hub, claim, &seed).await {
            Ok(unwritten) => unwritten,
            Err(error) => return vec![Item::unread(subject, format!("{error:#}"))],
        },
        None => Vec::new(),
    };
    let publish = Action::Publish {
        origin: origin.clone(),
        profile: hub.profile.clone(),
        declared: seed.clone(),
        hub: (hub.workspace.clone(), hub.sandbox.clone()),
    };
    let (active, item) = if unwritten.is_empty() {
        let Comparison {
            active, outcome, ..
        } = match compare_with_hub(&administrator, &seed).await {
            Ok(comparison) => comparison,
            Err(error) => return vec![Item::unread(subject, format!("{error:#}"))],
        };
        let Some(revision) = &active.revision else {
            return vec![Item::unread(
                subject,
                "Hub serves a control plane that is not its active revision, so nothing can be published over it; read again",
            )];
        };
        let subject = format!("control plane at {origin}, active revision {revision}");
        let item = match outcome {
            Outcome::Refused(reason) => Item::new(
                subject,
                State::Refused(format!(
                    "Hub refuses the declared control plane and would keep its active revision: {reason}"
                )),
            ),
            Outcome::Differs(differences) => {
                let restart = if reads_at_start(&differences) {
                    format!(
                        "; Hub reads {} only at its start, so apply restarts {}/{}",
                        READ_AT_START.join(", "),
                        hub.workspace,
                        hub.sandbox
                    )
                } else {
                    String::new()
                };
                Item::compared(
                    subject,
                    differences,
                    Some(format!(
                        "Hub's validated form of the declared document against the active revision, SHA-256 {}{restart}; people's grants, memberships, output owners and initial grants, repositories and assignments stay as they are",
                        active.sha256
                    )),
                )
                .with_action(publish)
            }
        };
        (active, item)
    } else {
        let active = match administrator.control_plane().await {
            Ok(active) => active,
            Err(error) => return vec![Item::unread(subject, format!("{error:#}"))],
        };
        let subject = match &active.revision {
            Some(revision) => format!("control plane at {origin}, active revision {revision}"),
            None => subject,
        };
        let unwritten = unwritten.join(", ");
        // The claim step writes them with its seed Job, which a manifest still holding a
        // render marker cannot make.
        let ready = claim
            .map(|claim| crate::kubernetes::seed_job(inputs, claim))
            .transpose();
        let item = match ready {
            Err(error) => Item::unusable(
                subject,
                &error.context(format!("the claim step cannot write {unwritten}")),
            ),
            Ok(_) => Item::deferred(
                subject,
                format!(
                    "Hub reads the files the declared control plane names when it validates it, and the claim step writes {unwritten} first; apply has Hub validate the document then, and publishes it unless Hub refuses it"
                ),
                publish,
            ),
        };
        (active, item)
    };
    let mut items = vec![item];
    items.extend(plan_registrations(inputs, &administrator, &origin, &hub.profile).await);
    if let Some(claim) = claim {
        items.push(plan_cleanup(inputs, claim, gateway, &seed, &active.control_plane).await);
    }
    items
}

/// The files of Hub's claim that the declared control plane names and Hub's sandbox
/// does not hold yet.
async fn unwritten_files(
    gateway: &Gateway,
    hub: &Hub,
    claim: &HubClaim,
    seed: &Value,
) -> Result<Vec<String>> {
    let named: Vec<String> = named_files(seed, &claim.directory)
        .into_iter()
        .map(|name| format!("{}/{name}", claim.directory))
        .collect();
    let held = gateway
        .digests(&hub.workspace, &hub.sandbox, &named)
        .await?;
    Ok(named
        .iter()
        .filter(|path| !held.contains_key(*path))
        .map(|path| path.rsplit('/').next().unwrap_or(path.as_str()).to_owned())
        .collect())
}

/// The deployment each registering sandbox declares, as `(server, deployment)`.
pub fn registering(inputs: &Inputs) -> Vec<(String, String, String)> {
    inputs
        .declaration
        .sandboxes()
        .filter_map(|sandbox| {
            let server = sandbox.registers.clone()?;
            let deployment = sandbox
                .environment
                .get(Sandbox::DEPLOYMENT_ID)
                .cloned()
                .or_else(|| {
                    inputs
                        .private
                        .sandbox_environment(&sandbox.key())
                        .and_then(|values| values.get(Sandbox::DEPLOYMENT_ID).cloned())
                })?;
            Some((sandbox.key(), server, deployment))
        })
        .collect()
}

/// The state Hub reports for one server deployment, `None` when it lists none.
pub fn health_of<'a>(report: &'a Value, server: &str, deployment: &str) -> Option<&'a str> {
    report["servers"].as_array()?.iter().find_map(|entry| {
        (entry["server"] == server && entry["deployment"] == deployment)
            .then(|| entry["state"].as_str().unwrap_or("unprobed"))
    })
}

async fn plan_registrations(
    inputs: &Inputs,
    administrator: &Administrator,
    origin: &Url,
    profile: &str,
) -> Vec<Item> {
    let expected = registering(inputs);
    if expected.is_empty() {
        return Vec::new();
    }
    let report = match administrator.server_health().await {
        Ok(report) => report,
        Err(error) => return vec![Item::unread("server registrations", format!("{error:#}"))],
    };
    expected
        .into_iter()
        .map(|(sandbox, server, deployment)| {
            let state = health_of(&report, &server, &deployment);
            let differences = if state == Some("healthy") {
                Vec::new()
            } else {
                vec![Difference::new(
                    ValuePath::root().member("state"),
                    Some(Value::String("healthy".to_owned())),
                    state.map(|state| Value::String(state.to_owned())),
                )]
            };
            Item::compared(
                format!("registration of {server} from {sandbox}"),
                differences,
                None,
            )
            .with_action(Action::AwaitRegistration {
                origin: origin.clone(),
                profile: profile.to_owned(),
                server,
                deployment,
            })
        })
        .collect()
}

/// Every claim path a control plane names.
pub fn named_files(control_plane: &Value, directory: &str) -> BTreeSet<String> {
    let mut named = BTreeSet::new();
    collect_named(control_plane, &format!("{directory}/"), &mut named);
    named
}

fn collect_named(value: &Value, prefix: &str, named: &mut BTreeSet<String>) {
    match value {
        Value::String(text) => {
            if let Some(name) = text.strip_prefix(prefix) {
                named.insert(name.to_owned());
            }
        }
        Value::Array(items) => items
            .iter()
            .for_each(|item| collect_named(item, prefix, named)),
        Value::Object(members) => members
            .values()
            .for_each(|member| collect_named(member, prefix, named)),
        _ => {}
    }
}

/// The claim's YAML files that the declared control plane does not name, which apply
/// removes once the new revision is active, each with what it is.
async fn plan_cleanup(
    inputs: &Inputs,
    claim: &HubClaim,
    gateway: &Gateway,
    declared: &Value,
    active: &Value,
) -> Item {
    let subject = format!(
        "files of {} no revision of this repository names",
        claim.directory
    );
    // Apply removes them with a Job made from the claim's seed Job.
    if let Err(error) = crate::kubernetes::seed_job(inputs, claim) {
        return Item::unusable(subject, &error);
    }
    let names = match gateway
        .list_directory(&claim.workspace, &claim.sandbox, &claim.directory)
        .await
    {
        Ok(names) => names,
        Err(error) => return Item::unread(subject, format!("{error:#}")),
    };
    let named = named_files(declared, &claim.directory);
    let used = named_files(active, &claim.directory);
    let sources: Vec<(String, String)> = claim_files(inputs, claim)
        .map(|files| {
            files
                .into_iter()
                .zip(&claim.files)
                .map(|((_, _, content), file)| (file.source.display().to_string(), content))
                .collect()
        })
        .unwrap_or_default();
    let mut differences = Vec::new();
    let mut remove = Vec::new();
    for name in names
        .iter()
        .filter(|name| is_claim_policy(name) && !named.contains(*name))
    {
        let path = format!("{}/{name}", claim.directory);
        let mut note = match gateway
            .read_file(&claim.workspace, &claim.sandbox, &path)
            .await
        {
            Ok(active) => match_source(&active, &sources),
            Err(error) => format!("not read: {error:#}"),
        };
        if used.contains(name) {
            note = format!("the active revision names it; {note}");
        }
        differences.push(Difference::new(
            ValuePath::root().member(name),
            None,
            Some(Value::String(note)),
        ));
        remove.push(name.clone());
    }
    let private: Vec<&str> = names
        .iter()
        .filter(|name| !is_claim_policy(name))
        .map(String::as_str)
        .collect();
    let note = (!private.is_empty()).then(|| format!("private files kept: {}", private.join(", ")));
    Item::compared(subject, differences, note).with_action(Action::RemoveClaimFiles {
        claim: claim.clone(),
        names: remove,
    })
}

/// A file this repository could have put in the claim: a YAML file with a plain name.
pub fn is_claim_policy(name: &str) -> bool {
    (name.ends_with(".yaml") || name.ends_with(".yml"))
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// A trusted YAML file against its repository source: equal, equal as YAML except
/// comments or formatting, or its differences.
pub fn file_state(declared: &str, active: &str, source: &Path) -> State {
    if declared == active {
        return State::Same;
    }
    match (parse_yaml(declared), parse_yaml(active)) {
        (Some(declared), Some(active)) => {
            let differences = compare(&declared, &active);
            let note = if differences.is_empty() {
                format!(
                    "differs from {} only in comments or formatting",
                    source.display()
                )
            } else {
                format!("differs from {}", source.display())
            };
            State::Differs {
                differences,
                note: Some(note),
            }
        }
        _ => State::Differs {
            differences: compare(
                &Value::String(declared.to_owned()),
                &Value::String(active.to_owned()),
            ),
            note: Some(format!("differs from {}", source.display())),
        },
    }
}

fn match_source(active: &str, sources: &[(String, String)]) -> String {
    let Some(active_value) = parse_yaml(active) else {
        return "it is not YAML".to_owned();
    };
    for (source, declared) in sources {
        if declared == active {
            return format!("its text equals {source}");
        }
        if parse_yaml(declared).is_some_and(|declared| compare(&declared, &active_value).is_empty())
        {
            return format!("it equals {source} except comments or formatting");
        }
    }
    "it matches no file this repository declares".to_owned()
}

fn parse_yaml(text: &str) -> Option<Value> {
    crate::yaml::value(text).ok()
}

// ----- apply -----

/// How many times a publication is made again when another revision became active.
const PUBLISH_ATTEMPTS: usize = 3;

/// Publishes the declared members over the revision active now, as its successor, and
/// checks that Hub then serves what it validated. When another revision becomes active
/// first, Hub answers 412 and nothing changes: the control plane is read, validated and
/// published again, from that revision, a bounded number of times.
pub async fn publish(
    inputs: &Inputs,
    origin: &Url,
    profile: &str,
    declared: &Value,
) -> Result<Published> {
    let administrator = Administrator::signed_in(inputs, origin, profile)?;
    publish_over(&administrator, declared).await
}

/// A publication that is active: its revision, and where Hub's form of it differs from the
/// revision it succeeded, none when another publication had made the same change.
#[derive(Debug)]
pub struct Published {
    pub revision: String,
    pub differences: Vec<Difference>,
}

impl Published {
    /// Whether a member Hub reads only at its start changed.
    pub fn read_at_start(&self) -> bool {
        reads_at_start(&self.differences)
    }
}

/// Whether a difference is in a section Hub reads only at its start.
pub fn reads_at_start(differences: &[Difference]) -> bool {
    differences.iter().any(|difference| {
        let path = difference.path.to_string();
        READ_AT_START
            .iter()
            .any(|section| path == *section || path.starts_with(&format!("{section}.")))
    })
}

/// The control plane route, as a publication reads, validates and writes it.
pub(crate) trait ControlPlaneRoute {
    async fn read(&self) -> Result<ActiveControlPlane>;
    async fn validate(&self, document: &Value) -> Result<Validation>;
    async fn write(&self, document: &Value, based_on: &str) -> Result<Publication>;
}

impl ControlPlaneRoute for Administrator {
    async fn read(&self) -> Result<ActiveControlPlane> {
        self.control_plane().await
    }

    async fn validate(&self, document: &Value) -> Result<Validation> {
        Administrator::validate(self, document).await
    }

    async fn write(&self, document: &Value, based_on: &str) -> Result<Publication> {
        self.publish(document, based_on).await
    }
}

/// The differing paths, for a message.
fn paths(differences: &[Difference]) -> String {
    differences
        .iter()
        .map(|difference| difference.path.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) async fn publish_over(
    administrator: &impl ControlPlaneRoute,
    declared: &Value,
) -> Result<Published> {
    let mut published = None;
    for attempt in 1..=PUBLISH_ATTEMPTS {
        let Comparison {
            active,
            document,
            outcome,
        } = compare_with_hub(administrator, declared).await?;
        let based_on = active
            .revision
            .clone()
            .context("Hub names no settled active revision to publish over; try again")?;
        let differences = match outcome {
            Outcome::Refused(reason) => bail!(
                "Hub refuses the declared control plane and keeps its active revision {based_on}: {reason}"
            ),
            Outcome::Differs(differences) => differences,
        };
        if differences.is_empty() {
            // Another publication made the same change.
            published = Some(Published {
                revision: based_on,
                differences,
            });
            break;
        }
        match administrator
            .write(&document, &based_on)
            .await
            .context("Hub refused the publication and keeps its active revision")?
        {
            Publication::Active(revision) => {
                published = Some(Published {
                    revision,
                    differences,
                });
                break;
            }
            Publication::Moved => {
                println!(
                    "   revision {based_on} is no longer active (attempt {attempt}); reading again"
                );
            }
        }
    }
    let Some(published) = published else {
        bail!(
            "another revision became active before each of {PUBLISH_ATTEMPTS} publications; nothing was published"
        );
    };
    let now = compare_with_hub(administrator, declared).await?;
    match now.outcome {
        Outcome::Differs(left) if left.is_empty() => Ok(published),
        Outcome::Differs(left) => bail!(
            "after publishing, the active revision {} still differs at {}",
            now.active.revision.unwrap_or_default(),
            paths(&left)
        ),
        Outcome::Refused(reason) => bail!(
            "after publishing, Hub refuses the declared control plane over revision {}: {reason}",
            now.active.revision.unwrap_or_default()
        ),
    }
}

/// Waits until Hub reports the server's deployment healthy.
pub async fn await_registration(
    inputs: &Inputs,
    origin: &Url,
    profile: &str,
    server: &str,
    deployment: &str,
) -> Result<()> {
    let administrator = Administrator::signed_in(inputs, origin, profile)?;
    let started = Instant::now();
    loop {
        let report = administrator.server_health().await?;
        let state = health_of(&report, server, deployment).map(str::to_owned);
        if state.as_deref() == Some("healthy") {
            return Ok(());
        }
        if started.elapsed() > REGISTRATION_TIMEOUT {
            bail!(
                "Hub reports {server} deployment {deployment} as {} after {} s",
                state.as_deref().unwrap_or("not registered"),
                REGISTRATION_TIMEOUT.as_secs()
            );
        }
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
}

/// Waits until Hub serves its public issuer and the Computer version.
pub async fn await_serving(origin: &Url, timeout: Duration) -> Result<()> {
    let http = reqwest::Client::builder()
        .user_agent(concat!("veoveo-installer/", env!("CARGO_PKG_VERSION")))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let issuer = origin.as_str().trim_end_matches('/').to_owned();
    let started = Instant::now();
    let mut last = String::new();
    while started.elapsed() < timeout {
        let discovery = http
            .get(origin.join(".well-known/oauth-authorization-server")?)
            .timeout(Duration::from_secs(15))
            .send()
            .await;
        match discovery {
            Ok(response) if response.status().is_success() => {
                let body: Value = response.json().await.unwrap_or_default();
                if body["issuer"]
                    .as_str()
                    .map(|text| text.trim_end_matches('/'))
                    == Some(issuer.as_str())
                {
                    let version = http
                        .get(origin.join("computer/version")?)
                        .timeout(Duration::from_secs(15))
                        .send()
                        .await;
                    match version {
                        Ok(response) if response.status().is_success() => return Ok(()),
                        Ok(response) => {
                            last = format!("/computer/version answered HTTP {}", response.status())
                        }
                        Err(error) => last = format!("/computer/version: {error}"),
                    }
                } else {
                    last = format!("discovery names the issuer {:?}", body["issuer"]);
                }
            }
            Ok(response) => last = format!("discovery answered HTTP {}", response.status()),
            Err(error) => last = format!("discovery: {error}"),
        }
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
    bail!(
        "Hub does not serve at {origin} after {} s: {last}",
        timeout.as_secs()
    )
}

/// The declared Hub step and Hub's public origin.
pub fn declared_origin(inputs: &Inputs) -> Result<(Url, &Hub)> {
    let hub = hub_step(inputs)?;
    Ok((origin(&declared_seed(inputs, hub)?, &hub.profile)?, hub))
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::*;

    fn seed() -> Value {
        let mut seed = json!({});
        merge_seed(
            &mut seed,
            json!({
                "profiles": [{"id": "local", "protectedResource": "https://veoveo.example.com/mcp/local"}],
                "oauthClients": [{"id": "entra-browser"}],
                "authorization": {"grants": [{"subject": {"principal": "55555555"}, "role": "admin"}], "roles": []},
                "workContexts": [
                    {"id": "reference-work", "memberships": [{"principals": ["44444444"]}]},
                    {"id": "new-work", "memberships": [{"principals": ["44444444"]}]},
                ],
            }),
        );
        merge_seed(
            &mut seed,
            json!({
                "oauthClients": [{"id": "veoveo-hub-sdk-cli"}],
                "computers": {"policyFile": "/etc/veoveo/hub/computer-policy-0123456789ab.yaml", "handbook": "# Handbook\nUse Lore.\nUse the Hub CLI.\n"},
                "factory": {"policyFile": "/etc/veoveo/hub/factory-base-ba9876543210.yaml"},
                "lore": {"deployments": [{"id": "primary"}], "repositories": [], "assignments": []},
            }),
        );
        seed
    }

    /// A recorded active control plane: the seed after people created a repository,
    /// were granted it and joined the Work Context, with the handbook of an earlier
    /// release and the policy files an operator named by hand.
    fn active() -> Value {
        let mut active = seed();
        active["authorization"]["grants"] =
            json!([{"subject": {"principal": "e8c1"}, "role": "admin"}]);
        active["workContexts"] =
            json!([{"id": "reference-work", "memberships": [{"principals": ["e8c1", "77aa"]}]}]);
        active["lore"]["repositories"] =
            json!([{"id": "01a1586c89057ea3adcfc848da54077b", "title": "time"}]);
        active["lore"]["assignments"] =
            json!([{"repository": "01a1586c89057ea3adcfc848da54077b", "principal": "e8c1"}]);
        active["computers"]["handbook"] = json!("# Handbook\nUse Lore.\n");
        active["computers"]["policyFile"] = json!("/etc/veoveo/hub/computer-policy-v3.yaml");
        active["factory"]["policyFile"] = json!("/etc/veoveo/hub/factory-base-v2.yaml");
        active
    }

    #[test]
    fn seed_documents_merge_by_their_typed_keys() {
        let clients: Vec<_> = seed()["oauthClients"]
            .as_array()
            .unwrap()
            .iter()
            .map(|client| client["id"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(clients, ["entra-browser", "veoveo-hub-sdk-cli"]);
        // The K3s fragment extends an entry the identity example gives: one entry, merged.
        let mut merged = json!({"oidcClients": [{"id": "entra", "allowedResources": ["local"], "metadata": {}}]});
        merge_seed(
            &mut merged,
            json!({"oidcClients": [{"id": "entra", "allowedResources": ["computer", "local"]}], "computers": {"port": 4096}}),
        );
        assert_eq!(
            merged,
            json!({
                "oidcClients": [{"id": "entra", "allowedResources": ["computer", "local"], "metadata": {}}],
                "computers": {"port": 4096},
            })
        );
    }

    #[test]
    fn the_origin_is_the_profiles_protected_resource_origin() {
        assert_eq!(
            origin(&seed(), "local").unwrap().as_str(),
            "https://veoveo.example.com/"
        );
        assert!(origin(&seed(), "computer").is_err());
    }

    #[test]
    fn what_people_created_is_not_a_difference() {
        let differences = compare(&installation_owned(seed()), &installation_owned(active()));
        let paths: Vec<String> = differences
            .iter()
            .map(|difference| difference.path.to_string())
            .collect();
        assert_eq!(
            paths,
            [
                "computers.handbook",
                "computers.policyFile",
                "factory.policyFile",
                "workContexts[id=new-work]"
            ]
        );
    }

    #[test]
    fn a_work_contexts_output_owner_and_initial_grants_are_the_active_revisions() {
        let mut declared = seed();
        declared["workContexts"][0]["outputPolicy"] = json!({
            "owner": {"kind": "principal", "id": "https://login.example/v2.0#55555555"},
            "initialGrants": [{"subject": {"kind": "principal", "id": "https://login.example/v2.0#44444444"}, "level": "viewer"}],
            "classification": "internal",
        });
        declared["workContexts"][1]["outputPolicy"] = json!({
            "owner": {"kind": "principal", "id": "https://login.example/v2.0#55555555"},
        });
        let mut active = active();
        active["workContexts"][0]["outputPolicy"] = json!({
            "owner": {"kind": "principal", "id": "https://login.example/v2.0#e8c1"},
            "initialGrants": [{"subject": {"kind": "principal", "id": "https://login.example/v2.0#77aa"}, "level": "editor"}],
            "classification": "restricted",
        });
        let document = publication(&declared, &active);
        // The person who owns the outputs, and the grants people made, as active; the
        // installation's classification as declared.
        assert_eq!(
            document["workContexts"][0]["outputPolicy"],
            json!({
                "owner": active["workContexts"][0]["outputPolicy"]["owner"],
                "initialGrants": active["workContexts"][0]["outputPolicy"]["initialGrants"],
                "classification": "internal",
            })
        );
        // Neither is a difference the installation would publish.
        let paths: Vec<String> = compare(
            &installation_owned(declared.clone()),
            &installation_owned(active.clone()),
        )
        .iter()
        .map(|difference| difference.path.to_string())
        .collect();
        assert!(
            !paths
                .iter()
                .any(|path| path.contains("owner") || path.contains("initialGrants")),
            "{paths:?}"
        );
        // A Work Context the active revision lacks has no owner to publish, never the
        // repository's example.
        assert_eq!(document["workContexts"][1]["outputPolicy"], json!({}));
        assert!(!document.to_string().contains("55555555"));
        assert_eq!(unowned_contexts(&document), ["new-work"]);
        // Once the active revision has it, its owner is carried too.
        active["workContexts"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id": "new-work", "memberships": [], "outputPolicy": {"owner": {"kind": "principal", "id": "https://login.example/v2.0#e8c1"}}}));
        assert!(unowned_contexts(&publication(&declared, &active)).is_empty());
    }

    #[test]
    fn a_publication_keeps_what_people_created_and_publishes_no_example_person() {
        let document = publication(&seed(), &active());
        // People's members as active.
        assert_eq!(
            document["authorization"]["grants"],
            active()["authorization"]["grants"]
        );
        assert_eq!(
            document["lore"]["repositories"],
            active()["lore"]["repositories"]
        );
        assert_eq!(
            document["lore"]["assignments"],
            active()["lore"]["assignments"]
        );
        assert_eq!(
            document["workContexts"][0]["memberships"],
            active()["workContexts"][0]["memberships"]
        );
        // A new Work Context gets no member; the seed's example person is not published.
        assert_eq!(
            document["workContexts"][1],
            json!({"id": "new-work", "memberships": []})
        );
        // The installation's members as declared.
        assert_eq!(document["computers"], seed()["computers"]);
        assert_eq!(
            installation_owned(document.clone()),
            installation_owned(seed())
        );
        // Publishing it leaves the active revision equal to the declaration.
        assert!(compare(&installation_owned(seed()), &installation_owned(document)).is_empty());
    }

    /// A claim whose seed Job's manifest still holds a render marker is not ready: plan
    /// does not compare it, as it does not compare any other marker file, instead of
    /// planning a write that would fail when apply makes it.
    #[test]
    fn a_claim_whose_manifest_holds_a_marker_is_not_compared() {
        let repository =
            std::env::temp_dir().join(format!("veoveo-installer-claim-{}", std::process::id()));
        std::fs::create_dir_all(repository.join("kubernetes")).unwrap();
        std::fs::write(repository.join("policy.yaml"), "version: 1\n").unwrap();
        let declaration = crate::declaration::Declaration::parse(
            r#"
            [[step]]
            name = "hub-claim"
            [step.claim]
            workspace = "vv-control"
            sandbox = "hub"
            directory = "/etc/veoveo/hub"
            manifest = "kubernetes/hub-config-pvc.yaml"
            job = "seed-hub-config-v1"
            secret = "hub-config-bootstrap"
            [[step.claim.file]]
            name = "computer-policy"
            source = "policy.yaml"
            "#,
        )
        .unwrap();
        let inputs = Inputs {
            repository: repository.clone(),
            declaration,
            lock: crate::inputs::tests::lock(),
            private: Default::default(),
        };
        let claim = inputs.declaration.claim("hub-claim").unwrap();
        let planned = |image: &str| {
            std::fs::write(
                repository.join("kubernetes/hub-config-pvc.yaml"),
                format!(
                    "apiVersion: batch/v1\nkind: Job\nmetadata:\n  name: seed-hub-config-v1\n  \
                     namespace: openshell-openshell-vv-control\nspec:\n  template:\n    spec:\n      \
                     containers:\n      - name: copy\n        image: {image}\n"
                ),
            )
            .unwrap();
            declared_claim(&inputs, claim)
                .err()
                .map(|error| Item::unusable("Hub's trusted files in /etc/veoveo/hub", &error))
        };
        let marker = planned("REPLACE_WITH_REVIEWED_COPY_IMAGE_AT_SHA256").unwrap();
        assert!(matches!(marker.state, State::NotCompared(_)), "{marker:?}");
        assert!(marker.matches() && marker.blocker().is_none() && marker.action.is_none());
        // An image the lock does not name is an error, which blocks apply.
        let unlocked = planned("@image:copy@").unwrap();
        assert!(matches!(unlocked.state, State::Unusable(_)), "{unlocked:?}");
        assert!(unlocked.blocker().is_some());
        // Filled from the lock, the claim is declared and plan compares it.
        assert!(planned("@image:lore@").is_none());
        std::fs::remove_dir_all(&repository).unwrap();
    }

    #[test]
    fn claim_files_are_named_by_their_content() {
        let a = claim_file_name("factory-base", "version: 1\n");
        let digest = a
            .strip_prefix("factory-base-")
            .and_then(|rest| rest.strip_suffix(".yaml"))
            .unwrap();
        assert_eq!(digest.len(), 12);
        assert!(
            digest
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
        assert_eq!(claim_file_name("factory-base", "version: 1\n"), a);
        assert_ne!(
            claim_file_name("factory-base", "version: 1\n# a comment\n"),
            a
        );
    }

    #[test]
    fn the_seed_names_claim_files_by_their_content() {
        let files = vec![(
            "computer-policy".to_owned(),
            "computer-policy-0123456789ab.yaml".to_owned(),
            "version: 1\n".to_owned(),
        )];
        assert_eq!(
            fill_claim(
                r#"{"policyFile": "@claim:computer-policy@"}"#,
                "/etc/veoveo/hub",
                &files
            )
            .unwrap(),
            r#"{"policyFile": "/etc/veoveo/hub/computer-policy-0123456789ab.yaml"}"#
        );
        let error = fill_claim(r#""@claim:factory-base@""#, "/etc/veoveo/hub", &files).unwrap_err();
        assert_eq!(error.to_string(), "the claim declares no file factory-base");
    }

    #[test]
    fn only_files_no_declared_revision_names_are_removed() {
        let named = named_files(&seed(), "/etc/veoveo/hub");
        assert_eq!(
            named.into_iter().collect::<Vec<_>>(),
            [
                "computer-policy-0123456789ab.yaml",
                "factory-base-ba9876543210.yaml"
            ]
        );
        assert!(is_claim_policy("computer-policy-v3.yaml"));
        assert!(!is_claim_policy("hub.env"));
        assert!(!is_claim_policy("authorization-server.jwks.json"));
        assert!(!is_claim_policy("../x.yaml"));
    }

    #[test]
    fn a_trusted_file_that_differs_only_in_comments_says_so() {
        let declared =
            "# Hub's host is veoveo.bioma.ai.\nversion: 1\nread_only: [/usr, /opt/veoveo/lore]\n";
        let active = "# Replace veoveo.bioma.ai with Hub's host.\nversion: 1\nread_only: [/usr, /opt/veoveo/lore]\n";
        let State::Differs { differences, note } = file_state(
            declared,
            active,
            Path::new("kubernetes/policies/factory-base.yaml"),
        ) else {
            panic!("the texts differ");
        };
        assert!(differences.is_empty());
        assert_eq!(
            note.unwrap(),
            "differs from kubernetes/policies/factory-base.yaml only in comments or formatting"
        );
        assert_eq!(
            match_source(
                active,
                &[(
                    "kubernetes/policies/factory-base.yaml".to_owned(),
                    declared.to_owned()
                )]
            ),
            "it equals kubernetes/policies/factory-base.yaml except comments or formatting"
        );
    }

    #[test]
    fn registration_reads_the_declared_deployment() {
        // A recorded server-health answer.
        let report = json!({"servers": [
            {"server": "console", "deployment": "0199a0a0-0000-7000-8000-000000000001", "state": "healthy", "checkedAt": "2026-10-25T07:00:00Z"},
            {"server": "media", "deployment": "0199a0a0-0000-7000-8000-000000000002", "state": null, "checkedAt": null},
            {"server": "media", "deployment": "0199a0a0-0000-7000-8000-000000000003", "state": "offline", "checkedAt": "2026-10-25T07:00:00Z"},
        ]});
        assert_eq!(
            health_of(&report, "console", "0199a0a0-0000-7000-8000-000000000001"),
            Some("healthy")
        );
        assert_eq!(
            health_of(&report, "media", "0199a0a0-0000-7000-8000-000000000002"),
            Some("unprobed")
        );
        assert_eq!(
            health_of(&report, "media", "0199a0a0-0000-7000-8000-000000000003"),
            Some("offline")
        );
        assert_eq!(
            health_of(&report, "artifact", "0199a0a0-0000-7000-8000-000000000004"),
            None
        );
    }
}

#[cfg(test)]
mod publication_tests {
    use std::sync::Mutex;

    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::*;

    /// Hub's form of a document, as recorded from Hub: every OAuth client's surface is
    /// written, `full_mcp` by default, and endpoints end in `/`.
    fn kept(document: &Value) -> Value {
        let mut kept = document.clone();
        if let Some(clients) = kept["oauthClients"].as_array_mut() {
            for client in clients {
                client
                    .as_object_mut()
                    .unwrap()
                    .entry("clientSurface")
                    .or_insert(json!("full_mcp"));
            }
        }
        if let Some(endpoint) = kept["computers"]["gateway"]["endpoint"].as_str()
            && !endpoint.ends_with('/')
        {
            kept["computers"]["gateway"]["endpoint"] = json!(format!("{endpoint}/"));
        }
        kept
    }

    fn digest(document: &Value) -> String {
        sha256_hex(&serde_json::to_string(document).unwrap())
    }

    /// Hub's control plane route as recorded states: the active revisions in order, kept
    /// in Hub's form; a person who creates a repository after the first read, before the
    /// publication; and a reason Hub refuses every document for.
    struct Route {
        revisions: Mutex<Vec<(String, Value)>>,
        reads: Mutex<usize>,
        writes: Mutex<usize>,
        concurrent_after_read: Option<Value>,
        refusal: Option<String>,
    }

    impl Route {
        fn over(active: Value) -> Self {
            Self {
                revisions: Mutex::new(vec![("gcp-1".into(), kept(&active))]),
                reads: Mutex::new(0),
                writes: Mutex::new(0),
                concurrent_after_read: None,
                refusal: None,
            }
        }
    }

    impl ControlPlaneRoute for Route {
        async fn read(&self) -> Result<ActiveControlPlane> {
            *self.reads.lock().unwrap() += 1;
            let revisions = self.revisions.lock().unwrap();
            let (revision, control_plane) = revisions.last().unwrap().clone();
            Ok(ActiveControlPlane {
                revision: Some(revision),
                sha256: digest(&control_plane),
                control_plane,
            })
        }

        async fn validate(&self, document: &Value) -> Result<Validation> {
            if let Some(reason) = &self.refusal {
                return Ok(Validation::Refused(reason.clone()));
            }
            let control_plane = kept(document);
            Ok(Validation::Valid {
                sha256: digest(&control_plane),
                control_plane,
            })
        }

        async fn write(&self, document: &Value, based_on: &str) -> Result<Publication> {
            *self.writes.lock().unwrap() += 1;
            let mut revisions = self.revisions.lock().unwrap();
            if let Some(created) = &self.concurrent_after_read
                && *self.reads.lock().unwrap() == 1
                && revisions.len() == 1
            {
                // The repository is created between the installer's read and its write.
                revisions.push(("gcp-2".into(), kept(created)));
            }
            if revisions.last().unwrap().0 != based_on {
                return Ok(Publication::Moved);
            }
            let made = format!("gcp-{}", revisions.len() + 1);
            revisions.push((made.clone(), kept(document)));
            Ok(Publication::Active(made))
        }
    }

    fn active() -> Value {
        json!({
            "computers": {"handbook": "old", "gateway": {"endpoint": "https://openshell.openshell.svc.cluster.local:8080"}},
            "oauthClients": [{"id": "veoveo-hub-sdk-cli", "grantTypes": ["authorization_code_pkce"]}],
            "authorization": {"grants": [{"role": "admin", "subject": {"principal": "person-1"}}], "roles": []},
            "workContexts": [],
            "lore": {"deployments": [], "repositories": [], "assignments": []},
        })
    }

    #[tokio::test]
    async fn the_declared_document_matches_hubs_form_of_it() {
        // The repository writes no defaults and no trailing `/`; Hub serves both.
        let route = Route::over(active());
        let mut declared = active();
        declared["authorization"]["grants"] = json!([{"role": "example"}]);
        let comparison = compare_with_hub(&route, &declared).await.unwrap();
        assert_ne!(comparison.active.control_plane, active());
        assert_eq!(comparison.outcome, Outcome::Differs(Vec::new()));
        // Nothing to publish: publication reads, validates and writes nothing.
        assert_eq!(
            publish_over(&route, &declared).await.unwrap().revision,
            "gcp-1"
        );
        assert_eq!(*route.writes.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn a_change_differs_where_hubs_forms_differ_and_a_second_publication_changes_nothing() {
        let route = Route::over(active());
        let mut declared = active();
        declared["computers"]["handbook"] = json!("new");
        let comparison = compare_with_hub(&route, &declared).await.unwrap();
        let Outcome::Differs(differences) = comparison.outcome else {
            panic!("Hub validates the document");
        };
        assert_eq!(
            differences
                .iter()
                .map(|difference| difference.path.to_string())
                .collect::<Vec<_>>(),
            ["computers.handbook"]
        );
        assert_eq!(
            publish_over(&route, &declared).await.unwrap().revision,
            "gcp-2"
        );
        assert_eq!(
            publish_over(&route, &declared).await.unwrap().revision,
            "gcp-2"
        );
        assert_eq!(*route.writes.lock().unwrap(), 1);
        let revisions = route.revisions.lock().unwrap();
        assert_eq!(revisions.last().unwrap().1["computers"]["handbook"], "new");
        // The person's grant is kept, never the seed's example.
        assert_eq!(
            revisions.last().unwrap().1["authorization"]["grants"],
            active()["authorization"]["grants"]
        );
    }

    #[tokio::test]
    async fn hub_restarts_only_for_a_member_it_reads_at_its_start() {
        let mut started = active();
        started["factory"] =
            json!({"policyFile": "/etc/veoveo/hub/factory-base-111111111111.yaml"});
        let route = Route::over(started.clone());
        let mut handbook = started.clone();
        handbook["computers"]["handbook"] = json!("new");
        assert!(
            !publish_over(&route, &handbook)
                .await
                .unwrap()
                .read_at_start()
        );
        let mut factory = handbook;
        factory["factory"]["policyFile"] = json!("/etc/veoveo/hub/factory-base-222222222222.yaml");
        let published = publish_over(&route, &factory).await.unwrap();
        assert!(published.read_at_start());
        assert_eq!(published.differences.len(), 1);
        assert!(
            !publish_over(&route, &factory)
                .await
                .unwrap()
                .read_at_start()
        );
    }

    #[tokio::test]
    async fn a_work_context_without_its_owner_is_refused_before_hub_validates_it() {
        let route = Route::over(active());
        let mut declared = active();
        declared["workContexts"] = json!([{"id": "new-work", "memberships": [],
            "outputPolicy": {"owner": {"kind": "principal", "id": "https://login.example/v2.0#55555555"}}}]);
        let comparison = compare_with_hub(&route, &declared).await.unwrap();
        let Outcome::Refused(reason) = comparison.outcome else {
            panic!("an owner-less Work Context is refused");
        };
        assert!(reason.contains("new-work"), "{reason}");
        assert!(!comparison.document.to_string().contains("55555555"));
        let error = publish_over(&route, &declared).await.unwrap_err();
        assert!(error.to_string().contains("example owner"), "{error}");
        assert_eq!(*route.writes.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn a_document_hub_refuses_is_never_written() {
        let mut route = Route::over(active());
        route.refusal = Some("duplicate policy rule `a` in policy `b`".into());
        let mut declared = active();
        declared["computers"]["handbook"] = json!("new");
        let comparison = compare_with_hub(&route, &declared).await.unwrap();
        assert_eq!(
            comparison.outcome,
            Outcome::Refused("duplicate policy rule `a` in policy `b`".into())
        );
        let error = publish_over(&route, &declared).await.unwrap_err();
        assert!(
            error.to_string().contains("duplicate policy rule"),
            "{error}"
        );
        assert_eq!(*route.writes.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn a_repository_created_while_publishing_survives_the_publication() {
        let mut declared = active();
        declared["computers"]["handbook"] = json!("new");
        declared["authorization"]["grants"] = json!([{"role": "example"}]);
        let mut created = active();
        created["lore"]["repositories"] = json!([{"id": "01a1586c89057ea3adcfc848da54077b"}]);
        let mut route = Route::over(active());
        route.concurrent_after_read = Some(created);
        let revision = publish_over(&route, &declared).await.unwrap().revision;
        let revisions = route.revisions.lock().unwrap();
        // The first write was refused (412); the second was based on the new revision.
        assert_eq!(revision, "gcp-3");
        let (_, published) = revisions.last().unwrap();
        assert_eq!(published["computers"]["handbook"], "new");
        assert_eq!(
            published["lore"]["repositories"],
            json!([{"id": "01a1586c89057ea3adcfc848da54077b"}])
        );
        assert_eq!(
            published["authorization"]["grants"],
            active()["authorization"]["grants"]
        );
    }

    #[tokio::test]
    async fn a_publication_that_keeps_losing_stops_without_publishing() {
        struct Busy;
        impl ControlPlaneRoute for Busy {
            async fn read(&self) -> Result<ActiveControlPlane> {
                Ok(ActiveControlPlane {
                    revision: Some("gcp-1".into()),
                    sha256: digest(&active()),
                    control_plane: active(),
                })
            }
            async fn validate(&self, document: &Value) -> Result<Validation> {
                Ok(Validation::Valid {
                    sha256: digest(document),
                    control_plane: document.clone(),
                })
            }
            async fn write(&self, _: &Value, _: &str) -> Result<Publication> {
                Ok(Publication::Moved)
            }
        }
        let mut declared = active();
        declared["computers"]["handbook"] = json!("new");
        let error = publish_over(&Busy, &declared).await.unwrap_err();
        assert!(
            error.to_string().contains("nothing was published"),
            "{error}"
        );
    }

    #[test]
    fn a_refusal_reason_is_the_problems_detail() {
        assert_eq!(
            refusal_reason(
                r#"{"type":"https://veoveo.ai/problems/control-plane-invalid","title":"Invalid control plane","status":400,"detail":"duplicate policy rule `a` in policy `b`"}"#
            ),
            "duplicate policy rule `a` in policy `b`"
        );
        assert_eq!(
            refusal_reason("invalid hub control plane\n"),
            "invalid hub control plane"
        );
    }
}
