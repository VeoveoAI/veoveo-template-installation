//! Hub's control plane, read through Hub's administrative route as the operator, an
//! installation administrator signed in through the browser with the Hub SDK's sign-in.
//! It is compared with this repository's seed, filled with the handbook. The members
//! people change (their grants, memberships, repositories and assignments) are never the
//! installation's, so they are left out of the comparison and of every publication.
//!
//! Hub's trusted files that come from this repository, such as the policies Hub hands
//! to Computers and factory instances, are read in Hub's sandbox through native exec.

use std::{collections::BTreeMap, path::Path, time::Duration};

use anyhow::{Context, Result, anyhow, bail};
use serde_json::Value;
use url::Url;
use veoveo_hub_sdk::{HubProfileId, OAuthClientId, client::BearerSource as _};
use veoveo_hub_sdk_cli::{
    grant::{GrantBearer, GrantStore},
    hub::{HubAccess, HubAuthentication},
    sign_in::{SignIn, sign_in},
};
use veoveo_types::ScopeName;

use crate::{
    declaration::{Hub, HubClaim},
    difference::{compare, remove},
    inputs::Inputs,
    openshell::Gateway,
    plan::{Item, State},
};

/// The public client the installation registers for a terminal's browser sign-in.
pub const SIGN_IN_CLIENT: &str = "veoveo-hub-sdk-cli";
pub const SIGN_IN_REDIRECT: &str = "http://127.0.0.1:3211/callback";
pub const SIGN_IN_SCOPE: &str = "mcp";

/// Members of the control plane that people change, never the installation.
pub const PEOPLE: [&str; 4] = [
    "authorization.grants",
    "workContexts.*.memberships",
    "lore.repositories",
    "lore.assignments",
];

/// The seed documents merged by their typed keys, with the handbook.
pub fn declared_seed(inputs: &Inputs, hub: &Hub) -> Result<Value> {
    let mut seed = Value::Object(serde_json::Map::new());
    for file in &hub.seed {
        let document: Value = serde_json::from_str(&inputs.text(file)?)
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

/// What Hub's administrative route answers.
pub struct ActiveControlPlane {
    pub revision: Option<String>,
    pub control_plane: Value,
}

/// Reads the active control plane as the signed-in operator.
pub async fn read_control_plane(
    inputs: &Inputs,
    origin: &Url,
    profile: &str,
) -> Result<ActiveControlPlane> {
    let grants = grant_store(inputs)?;
    let access = access(origin, profile, &grants)?;
    let profile_id = HubProfileId::parse(profile)?;
    let http = access.oauth_http()?;
    let stored = grants.load(origin, &profile_id).ok().flatten();
    if stored.is_none() {
        bail!(
            "there is no stored Hub sign-in to {} for profile {profile} in {}; sign in as an installation administrator with `veoveo-installer login`",
            origin.origin().ascii_serialization(),
            grants.directory().display()
        );
    }
    let bearer = GrantBearer::load(grants.clone(), origin, &profile_id, http.clone()).map_err(|error| {
        let reason = error.to_string();
        let reason = reason.split("; ").next().unwrap_or_default().to_owned();
        anyhow!("the stored sign-in cannot be used: {reason}; sign in again with `veoveo-installer login`")
    })?;
    let mut address = origin.clone();
    address
        .path_segments_mut()
        .map_err(|()| anyhow!("{origin} cannot hold a path"))?
        .pop_if_empty()
        .extend(["admin", profile, "control-plane"]);
    for attempt in 0..2 {
        let token = bearer
            .caller_bearer()
            .await
            .map_err(|error| anyhow!("the sign-in could not be renewed: {error}"))?;
        let response = http
            .get(address.clone())
            .bearer_auth(token.expose())
            .timeout(Duration::from_secs(30))
            .send()
            .await
            .with_context(|| format!("cannot reach {address}"))?;
        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED && attempt == 0 {
            bearer.forget_caller();
            continue;
        }
        match status.as_u16() {
            200 => {}
            401 => bail!(
                "Hub does not admit the stored sign-in (HTTP 401); sign in again with `veoveo-installer login`"
            ),
            403 => bail!(
                "Hub refused the signed-in person (HTTP 403): reading the control plane needs an installation administrator"
            ),
            code => bail!("Hub answered HTTP {code} for {address}"),
        }
        let body: Value = response.json().await.context("Hub answered no JSON")?;
        let control_plane = body
            .get("controlPlane")
            .cloned()
            .context("Hub's answer holds no controlPlane")?;
        return Ok(ActiveControlPlane {
            revision: body["revisionId"].as_str().map(str::to_owned),
            control_plane,
        });
    }
    unreachable!("the second attempt returns")
}

pub async fn plan(inputs: &Inputs, hub: &Hub, gateway: Option<&Gateway>) -> Vec<Item> {
    let mut items = Vec::new();
    let seed = match declared_seed(inputs, hub) {
        Ok(seed) => seed,
        Err(error) => {
            return vec![Item::new(
                "control plane",
                State::NotCompared(format!("{error:#}")),
            )];
        }
    };
    let active = match origin(&seed, &hub.profile) {
        Ok(origin) => match read_control_plane(inputs, &origin, &hub.profile).await {
            Ok(active) => Some(active),
            Err(error) => {
                items.push(Item::unread(
                    format!("control plane at {origin}"),
                    format!("{error:#}"),
                ));
                None
            }
        },
        Err(error) => {
            items.push(Item::new(
                "control plane",
                State::NotCompared(format!("{error:#}")),
            ));
            None
        }
    };
    if let Some(active) = &active {
        let subject = match &active.revision {
            Some(revision) => format!("control plane, active revision {revision}"),
            None => "control plane".to_owned(),
        };
        let differences = compare(
            &installation_owned(seed),
            &installation_owned(active.control_plane.clone()),
        );
        items.push(Item::compared(
            subject,
            differences,
            Some("apply publishes these members through the administrative route; people's grants, memberships, repositories and assignments stay as they are".to_owned()),
        ));
    }
    if let Some(claim) = &hub.claim {
        items.extend(
            plan_claim(
                inputs,
                claim,
                gateway,
                active.as_ref().map(|active| &active.control_plane),
            )
            .await,
        );
    }
    items
}

async fn plan_claim(
    inputs: &Inputs,
    claim: &HubClaim,
    gateway: Option<&Gateway>,
    control_plane: Option<&Value>,
) -> Vec<Item> {
    let subject = |name: &str| format!("Hub's trusted file {}/{name}", claim.directory);
    let Some(gateway) = gateway else {
        return vec![Item::unread(
            format!("Hub's trusted files in {}", claim.directory),
            "the OpenShell gateway was not reached",
        )];
    };
    let names = match gateway
        .list_directory(&claim.workspace, &claim.sandbox, &claim.directory)
        .await
    {
        Ok(names) => names,
        Err(error) => {
            return vec![Item::unread(
                format!("Hub's trusted files in {}", claim.directory),
                format!("{error:#}"),
            )];
        }
    };
    let used = used_files(control_plane, &claim.directory);
    let mut sources: BTreeMap<&str, String> = BTreeMap::new();
    let mut items = Vec::new();
    for (name, file) in &claim.files {
        let declared = match inputs.filled(file) {
            Ok(text) => text,
            Err(error) => {
                items.push(Item::new(
                    subject(name),
                    State::NotCompared(format!("{error:#}")),
                ));
                continue;
            }
        };
        let item = if names.iter().any(|present| present == name) {
            let path = format!("{}/{name}", claim.directory);
            match gateway
                .read_file(&claim.workspace, &claim.sandbox, &path)
                .await
            {
                Ok(active) => Item::new(subject(name), file_state(&declared, &active, file)),
                Err(error) => Item::unread(subject(name), format!("{error:#}")),
            }
        } else {
            Item::new(subject(name), State::Absent { note: None })
        };
        items.push(with_use(item, used.get(name.as_str())));
        sources.insert(name, declared);
    }
    let (policies, private): (Vec<&String>, Vec<&String>) = names
        .iter()
        .filter(|name| !claim.files.contains_key(*name))
        .partition(|name| name.ends_with(".yaml") || name.ends_with(".yml"));
    if !private.is_empty() {
        let private: Vec<&str> = private.iter().map(|name| name.as_str()).collect();
        items.push(Item::new(
            format!("Hub's private trusted files {}", private.join(", ")),
            State::NotCompared(
                "they come from the private directory, not this repository".to_owned(),
            ),
        ));
    }
    for name in policies {
        let path = format!("{}/{name}", claim.directory);
        let note = match gateway
            .read_file(&claim.workspace, &claim.sandbox, &path)
            .await
        {
            Ok(active) => match_source(&active, claim, &sources),
            Err(error) => format!("not read: {error:#}"),
        };
        let note = match used.get(name.as_str()) {
            Some(member) => format!("the active control plane names it as {member}; {note}"),
            None => note,
        };
        items.push(Item::new(subject(name), State::Undeclared(note)));
    }
    items
}

/// The files of `directory` the active control plane names, by the member naming them.
fn used_files(control_plane: Option<&Value>, directory: &str) -> BTreeMap<String, String> {
    let mut used = BTreeMap::new();
    let Some(control_plane) = control_plane else {
        return used;
    };
    for (section, member) in [("computers", "policyFile"), ("factory", "policyFile")] {
        if let Some(path) = control_plane[section][member].as_str()
            && let Some(name) = path.strip_prefix(&format!("{directory}/"))
        {
            used.insert(name.to_owned(), format!("{section}.{member}"));
        }
    }
    used
}

fn with_use(mut item: Item, member: Option<&String>) -> Item {
    if let (Some(member), State::Differs { note, .. }) = (member, &mut item.state) {
        let used = format!("the active control plane names it as {member}");
        *note = Some(match note.take() {
            Some(existing) => format!("{existing}; {used}"),
            None => used,
        });
    }
    item
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

fn match_source(active: &str, claim: &HubClaim, sources: &BTreeMap<&str, String>) -> String {
    let Some(active_value) = parse_yaml(active) else {
        return "it is not YAML".to_owned();
    };
    for (name, declared) in sources {
        let source = claim.files[*name].display();
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
                "workContexts": [{"id": "reference-work", "memberships": [{"principals": ["44444444"]}]}],
            }),
        );
        merge_seed(
            &mut seed,
            json!({
                "oauthClients": [{"id": "veoveo-hub-sdk-cli"}],
                "computers": {"policyFile": "/etc/veoveo/hub/computer-policy-v1.yaml", "handbook": "# Handbook\nUse Lore.\nUse the Hub CLI.\n"},
                "lore": {"deployments": [{"id": "primary"}], "repositories": [], "assignments": []},
            }),
        );
        seed
    }

    #[test]
    fn seed_documents_merge_by_their_typed_keys() {
        let seed = seed();
        let clients: Vec<_> = seed["oauthClients"]
            .as_array()
            .unwrap()
            .iter()
            .map(|client| client["id"].as_str().unwrap())
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

    /// A recorded active control plane: the seed after people created a repository,
    /// were granted it and joined the Work Context, with the handbook of an earlier
    /// release and a Computer policy file published later.
    fn active() -> Value {
        let mut active = seed();
        active["authorization"]["grants"] =
            json!([{"subject": {"principal": "e8c1"}, "role": "admin"}]);
        active["workContexts"][0]["memberships"] = json!([{"principals": ["e8c1", "77aa"]}]);
        active["lore"]["repositories"] =
            json!([{"id": "01a1586c89057ea3adcfc848da54077b", "title": "time"}]);
        active["lore"]["assignments"] =
            json!([{"repository": "01a1586c89057ea3adcfc848da54077b", "principal": "e8c1"}]);
        active["computers"]["handbook"] = json!("# Handbook\nUse Lore.\n");
        active["computers"]["policyFile"] = json!("/etc/veoveo/hub/computer-policy-v3.yaml");
        active
    }

    #[test]
    fn what_people_created_is_not_a_difference() {
        let differences = compare(&installation_owned(seed()), &installation_owned(active()));
        let paths: Vec<String> = differences
            .iter()
            .map(|difference| difference.path.to_string())
            .collect();
        assert_eq!(paths, ["computers.handbook", "computers.policyFile"]);
        assert!(installation_owned(active())["lore"]["deployments"].is_array());
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
        let older = "version: 1\nread_only: [/usr]\n";
        let State::Differs { differences, .. } = file_state(declared, older, Path::new("p.yaml"))
        else {
            panic!("the policies differ");
        };
        assert_eq!(differences[0].path.to_string(), "read_only[1]");
        assert!(matches!(
            file_state(declared, declared, Path::new("p.yaml")),
            State::Same
        ));
    }

    #[test]
    fn files_the_active_control_plane_names_are_found() {
        let used = used_files(Some(&active()), "/etc/veoveo/hub");
        assert_eq!(
            used.get("computer-policy-v3.yaml").map(String::as_str),
            Some("computers.policyFile")
        );
        assert!(used_files(None, "/etc/veoveo/hub").is_empty());
    }
}
