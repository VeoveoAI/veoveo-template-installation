//! Hub's trusted files from the identity provider's registration: `hub.env` with Hub's
//! own keys and the supplied secrets, the authorization server's public key set, and the
//! seed `hub.json`. Keys are made once and never replaced: a run over existing files
//! recovers them, checks that they agree with what the private directory supplies and
//! refuses, without writing, when they do not. A rotation is a separate, reviewed act.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};

use crate::{
    hub as control_plane,
    identity::files::{
        authorization_jwk, base64, ed25519_key, parse_environment, private_directory, random_bytes,
        read_secret, render_environment, token_urlsafe, write_private,
    },
    inputs::Inputs,
    private::Identity,
};

pub const ENVIRONMENT_FILE: &str = "hub.env";
pub const KEY_SET_FILE: &str = "authorization-server.jwks.json";
pub const SEED_FILE: &str = "hub.json";

/// The variables of Hub's own keys, made once.
pub const GENERATED: [&str; 5] = [
    "VEOVEO_FORWARDED_TOKEN_KEY_B64",
    "VEOVEO_AUDIT_SIGNING_KEY_B64",
    "VEOVEO_REFRESH_DELIVERY_KEY_B64",
    "VEOVEO_REGISTRATION_SECRET",
    "VEOVEO_ACCESS_TOKEN_SIGNING_KEY_B64",
];
const ISSUER: &str = "VEOVEO_ISSUER";
const CONFIG: &str = "VEOVEO_CONFIG";
const SIGNING_KEY: &str = "VEOVEO_ACCESS_TOKEN_SIGNING_KEY_B64";

/// What a preparation found and did, by file; it names no value.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub lines: Vec<String>,
}

/// Where Hub's trusted files are kept.
pub fn directory(inputs: &Inputs) -> Result<(PathBuf, &Identity)> {
    let identity = inputs
        .private
        .identity
        .as_ref()
        .context("private.toml has no [identity] section")?;
    Ok((inputs.private.path(&identity.directory), identity))
}

/// Prepares, or with `check` only verifies, Hub's trusted files.
pub fn prepare(inputs: &Inputs, check: bool) -> Result<Report> {
    let (directory, identity) = directory(inputs)?;
    let hub = inputs
        .declaration
        .steps
        .iter()
        .find_map(|step| match &step.kind {
            crate::declaration::StepKind::Hub(hub) => Some(hub),
            _ => None,
        })
        .context("installation.toml declares no Hub step")?;
    let claim = inputs
        .declaration
        .claim(&hub.claim)
        .context("the Hub step names no claim")?;
    let seed = control_plane::declared_seed(inputs, hub)?;
    let origin = control_plane::origin(&seed, &hub.profile)?;
    let wanted = Wanted {
        issuer: origin.as_str().trim_end_matches('/').to_owned(),
        config: format!("{}/{SEED_FILE}", claim.directory),
        environment: identity.environment.clone(),
        secrets: identity
            .secrets
            .iter()
            .map(|(name, file)| (name.clone(), inputs.private.path(file)))
            .collect(),
    };
    let paths = Paths::new(&directory);
    let people = people(identity, inputs, &paths)?;
    prepare_in(&paths, &wanted, &seed, people, check)
}

/// What `hub.env` must hold besides Hub's own keys.
pub struct Wanted {
    pub issuer: String,
    pub config: String,
    pub environment: std::collections::BTreeMap<String, String>,
    pub secrets: Vec<(String, PathBuf)>,
}

pub struct Paths {
    pub directory: PathBuf,
    pub environment: PathBuf,
    pub key_set: PathBuf,
    pub seed: PathBuf,
}

impl Paths {
    pub fn new(directory: &Path) -> Self {
        Self {
            directory: directory.to_path_buf(),
            environment: directory.join(ENVIRONMENT_FILE),
            key_set: directory.join(KEY_SET_FILE),
            seed: directory.join(SEED_FILE),
        }
    }
}

/// The seed's people: the existing seed's, or the private people document's for a
/// first seed, or none.
fn people(identity: &Identity, inputs: &Inputs, paths: &Paths) -> Result<Option<Value>> {
    if paths.seed.exists() {
        let text = std::fs::read_to_string(&paths.seed)
            .with_context(|| format!("cannot read {}", paths.seed.display()))?;
        return Ok(Some(serde_json::from_str(&text).with_context(|| {
            format!("{} is not JSON", paths.seed.display())
        })?));
    }
    match &identity.people {
        Some(file) => {
            let path = inputs.private.path(file);
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("cannot read {}", path.display()))?;
            Ok(Some(serde_json::from_str(&text).with_context(|| {
                format!("{} is not JSON", path.display())
            })?))
        }
        None => Ok(None),
    }
}

pub fn prepare_in(
    paths: &Paths,
    wanted: &Wanted,
    seed: &Value,
    people: Option<Value>,
    check: bool,
) -> Result<Report> {
    let mut report = Report::default();
    let environment_exists = paths.environment.exists();
    if !environment_exists && (paths.key_set.exists() || paths.seed.exists()) {
        bail!(
            "{} is missing while its key set or seed is there: recover hub.env, whose keys they name; nothing is regenerated",
            paths.environment.display()
        );
    }
    let environment = if environment_exists {
        let text = std::fs::read_to_string(&paths.environment)
            .with_context(|| format!("cannot read {}", paths.environment.display()))?;
        let values = parse_environment(&text)
            .with_context(|| format!("{} is not valid", paths.environment.display()))?;
        recover(&values, wanted).with_context(|| {
            format!(
                "{} does not agree with this installation",
                paths.environment.display()
            )
        })?;
        report.lines.push(format!(
            "{}: kept, {} values; Hub's keys recovered",
            paths.environment.display(),
            values.len()
        ));
        values
    } else {
        let values = create(wanted)?;
        if check {
            report.lines.push(format!(
                "{}: would be created with new keys {}",
                paths.environment.display(),
                GENERATED.join(", ")
            ));
        } else {
            private_directory(&paths.directory)?;
            write_private(&paths.environment, &render_environment(&values), true)?;
            report.lines.push(format!(
                "{}: created with new keys {}",
                paths.environment.display(),
                GENERATED.join(", ")
            ));
        }
        values
    };
    let value = |name: &str| {
        environment
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    };
    let jwk =
        authorization_jwk(value(SIGNING_KEY).context("hub.env has no access-token signing key")?)?;
    let key_set = json!({"keys": [jwk.clone()]});
    if paths.key_set.exists() {
        let text = std::fs::read_to_string(&paths.key_set)?;
        let existing: Value = serde_json::from_str(&text)
            .with_context(|| format!("{} is not JSON", paths.key_set.display()))?;
        ensure!(
            existing == key_set,
            "{} is not the public key of hub.env's signing key: recover the key both came from",
            paths.key_set.display()
        );
        report.lines.push(format!(
            "{}: kept, key {}",
            paths.key_set.display(),
            jwk["kid"].as_str().unwrap_or_default()
        ));
    } else if check {
        report.lines.push(format!(
            "{}: would be written, key {}",
            paths.key_set.display(),
            jwk["kid"].as_str().unwrap_or_default()
        ));
    } else {
        write_private(
            &paths.key_set,
            &(serde_json::to_string_pretty(&key_set)? + "\n"),
            false,
        )?;
        report.lines.push(format!(
            "{}: written, key {}",
            paths.key_set.display(),
            jwk["kid"].as_str().unwrap_or_default()
        ));
    }
    let document = seed_document(
        seed,
        people.as_ref(),
        jwk["kid"].as_str().unwrap_or_default(),
    );
    missing_secrets(&document, &environment)?;
    let text = serde_json::to_string_pretty(&document)? + "\n";
    let unchanged = std::fs::read_to_string(&paths.seed)
        .ok()
        .is_some_and(|existing| existing == text);
    if unchanged {
        report.lines.push(format!("{}: kept", paths.seed.display()));
    } else if check {
        report.lines.push(format!(
            "{}: would be written from the repository's seed",
            paths.seed.display()
        ));
    } else {
        write_private(&paths.seed, &text, false)?;
        report.lines.push(format!(
            "{}: written from the repository's seed",
            paths.seed.display()
        ));
    }
    Ok(report)
}

/// The variables of a first `hub.env`: new keys, the supplied secrets and the settings.
fn create(wanted: &Wanted) -> Result<Vec<(String, String)>> {
    let mut values = vec![
        (
            "VEOVEO_FORWARDED_TOKEN_KEY_B64".to_owned(),
            base64(&ed25519_key()?),
        ),
        (
            "VEOVEO_AUDIT_SIGNING_KEY_B64".to_owned(),
            base64(&random_bytes(32)?),
        ),
        (
            "VEOVEO_REFRESH_DELIVERY_KEY_B64".to_owned(),
            base64(&random_bytes(32)?),
        ),
        ("VEOVEO_REGISTRATION_SECRET".to_owned(), token_urlsafe(32)?),
        (SIGNING_KEY.to_owned(), base64(&ed25519_key()?)),
    ];
    for (name, file) in &wanted.secrets {
        values.push((
            name.clone(),
            read_secret(file).with_context(|| format!("{name} is supplied in a file"))?,
        ));
    }
    values.push((ISSUER.to_owned(), wanted.issuer.clone()));
    values.push((CONFIG.to_owned(), wanted.config.clone()));
    for (name, value) in &wanted.environment {
        ensure!(
            !values.iter().any(|(existing, _)| existing == name),
            "{name} is set twice in private.toml"
        );
        values.push((name.clone(), value.clone()));
    }
    Ok(values)
}

/// An existing `hub.env` keeps its keys; it must hold every one, the origin and
/// settings this installation has, and every secret a file supplies must be its value.
pub fn recover(values: &[(String, String)], wanted: &Wanted) -> Result<()> {
    let value = |name: &str| {
        values
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    };
    let missing: Vec<&str> = GENERATED
        .iter()
        .copied()
        .filter(|name| value(name).is_none_or(str::is_empty))
        .collect();
    ensure!(
        missing.is_empty(),
        "it lacks {}: recover the existing Hub state; missing keys are never made again",
        missing.join(", ")
    );
    ensure!(
        value(ISSUER) == Some(wanted.issuer.as_str()),
        "its {ISSUER} is not {}: Hub's origin does not change under its keys",
        wanted.issuer
    );
    ensure!(
        value(CONFIG) == Some(wanted.config.as_str()),
        "its {CONFIG} is not {}",
        wanted.config
    );
    let mut differing = Vec::new();
    for (name, setting) in &wanted.environment {
        if value(name) != Some(setting.as_str()) {
            differing.push(name.as_str());
        }
    }
    ensure!(
        differing.is_empty(),
        "it holds other values of {} than private.toml: change them through a reviewed rotation",
        differing.join(", ")
    );
    for (name, file) in &wanted.secrets {
        match (value(name), file.exists()) {
            (Some(held), true) => ensure!(
                read_secret(file)? == held,
                "its {name} differs from {}: rotate it separately",
                file.display()
            ),
            (Some(_), false) => {}
            (None, _) => bail!(
                "it lacks {name}, which {} supplies: add it through a reviewed rotation",
                file.display()
            ),
        }
    }
    Ok(())
}

/// The seed: this repository's, with the access-token key's ID, and people's members
/// from `people` (an earlier seed or the people document), or none.
pub fn seed_document(seed: &Value, people: Option<&Value>, kid: &str) -> Value {
    let mut document = control_plane::publication(seed, people.unwrap_or(&json!({})));
    // People the earlier seed or the people document do not name are none, never the
    // repository's examples.
    for (section, member) in [
        ("authorization", "grants"),
        ("lore", "repositories"),
        ("lore", "assignments"),
    ] {
        if let Some(section) = document.get_mut(section).and_then(Value::as_object_mut) {
            section
                .entry(member.to_owned())
                .or_insert_with(|| json!([]));
        }
    }
    if let Some(servers) = document
        .get_mut("authorizationServers")
        .and_then(Value::as_array_mut)
    {
        for server in servers
            .iter_mut()
            .filter(|server| server.get("accessTokenSigningKey").is_some())
        {
            server["accessTokenKeyId"] = Value::String(kid.to_owned());
        }
    }
    document
}

/// Every environment secret the seed names is in `hub.env`.
fn missing_secrets(seed: &Value, environment: &[(String, String)]) -> Result<()> {
    let missing: Vec<&str> = seed["secrets"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|secret| secret["source"] == "env")
        .filter_map(|secret| secret["locator"].as_str())
        .filter(|locator| !environment.iter().any(|(name, _)| name == locator))
        .collect();
    ensure!(
        missing.is_empty(),
        "the seed names {}, which hub.env lacks: supply each in [identity.secrets]",
        missing.join(", ")
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    struct Fixture {
        directory: PathBuf,
        wanted: Wanted,
        seed: Value,
    }

    impl Fixture {
        fn new(name: &str) -> Self {
            let directory = std::env::temp_dir().join(format!(
                "veoveo-installer-identity-{name}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&directory);
            private_directory(&directory).unwrap();
            let secret = directory.join("entra-client.secret");
            std::fs::write(&secret, "fixture-client-secret").unwrap();
            let password = directory.join("db.password");
            std::fs::write(&password, "fixture-db-password").unwrap();
            Self {
                wanted: Wanted {
                    issuer: "https://veoveo.example.com".into(),
                    config: "/etc/veoveo/hub/hub.json".into(),
                    environment: [(
                        "VEOVEO_LISTEN_ADDRESS".to_owned(),
                        "127.0.0.1:8080".to_owned(),
                    )]
                    .into_iter()
                    .collect(),
                    secrets: vec![
                        ("VEOVEO_DATABASE_PASSWORD".into(), password),
                        ("VEOVEO_IDP_OIDC_CLIENT_SECRET".into(), secret),
                    ],
                },
                seed: json!({
                    "authorizationServers": [{"id": "hub", "accessTokenKeyId": "example", "accessTokenSigningKey": "access_token_signing_key"}],
                    "authorization": {"grants": [{"role": "admin", "subject": {"principal": "55555555"}}], "roles": []},
                    "workContexts": [{"id": "reference-work", "memberships": [{"principals": ["44444444"]}]}],
                    "lore": {"deployments": [], "repositories": [], "assignments": []},
                    "secrets": [
                        {"id": "access_token_signing_key", "source": "env", "locator": "VEOVEO_ACCESS_TOKEN_SIGNING_KEY_B64"},
                        {"id": "entra_client_secret", "source": "env", "locator": "VEOVEO_IDP_OIDC_CLIENT_SECRET"},
                    ],
                }),
                directory,
            }
        }

        fn paths(&self) -> Paths {
            Paths::new(&self.directory.join("hub-config"))
        }

        fn files(&self) -> Vec<(String, Vec<u8>)> {
            let mut files: Vec<_> = std::fs::read_dir(self.directory.join("hub-config"))
                .map(|entries| {
                    entries
                        .map(|entry| entry.unwrap())
                        .map(|entry| {
                            (
                                entry.file_name().into_string().unwrap(),
                                std::fs::read(entry.path()).unwrap(),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            files.sort();
            files
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.directory);
        }
    }

    #[test]
    fn a_first_preparation_makes_keys_once_and_a_rerun_keeps_every_byte() {
        let fixture = Fixture::new("first");
        let report = prepare_in(
            &fixture.paths(),
            &fixture.wanted,
            &fixture.seed,
            None,
            false,
        )
        .unwrap();
        assert!(
            report.lines[0].contains("created with new keys"),
            "{report:?}"
        );
        let first = fixture.files();
        assert_eq!(
            first
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>(),
            ["authorization-server.jwks.json", "hub.env", "hub.json"]
        );
        let environment = parse_environment(std::str::from_utf8(&first[1].1).unwrap()).unwrap();
        let names: Vec<&str> = environment.iter().map(|(name, _)| name.as_str()).collect();
        for name in GENERATED.iter().chain(&[
            "VEOVEO_DATABASE_PASSWORD",
            "VEOVEO_IDP_OIDC_CLIENT_SECRET",
            ISSUER,
            CONFIG,
            "VEOVEO_LISTEN_ADDRESS",
        ]) {
            assert!(names.contains(name), "{name}");
        }
        // The seed names the public key's ID and no example person.
        let seed: Value = serde_json::from_slice(&first[2].1).unwrap();
        let key_set: Value = serde_json::from_slice(&first[0].1).unwrap();
        assert_eq!(
            seed["authorizationServers"][0]["accessTokenKeyId"],
            key_set["keys"][0]["kid"]
        );
        assert_eq!(seed["authorization"]["grants"], json!([]));
        assert_eq!(seed["workContexts"][0]["memberships"], json!([]));
        assert!(
            !String::from_utf8_lossy(&first[0].1).contains(
                &environment
                    .iter()
                    .find(|(n, _)| n == SIGNING_KEY)
                    .unwrap()
                    .1
            )
        );
        // A rerun changes nothing.
        let report = prepare_in(
            &fixture.paths(),
            &fixture.wanted,
            &fixture.seed,
            Some(seed),
            false,
        )
        .unwrap();
        assert!(
            report.lines.iter().all(|line| line.contains("kept")),
            "{report:?}"
        );
        assert_eq!(fixture.files(), first);
    }

    #[test]
    fn a_supplied_secret_that_differs_is_refused_without_writing() {
        let fixture = Fixture::new("differs");
        prepare_in(
            &fixture.paths(),
            &fixture.wanted,
            &fixture.seed,
            None,
            false,
        )
        .unwrap();
        let before = fixture.files();
        std::fs::write(&fixture.wanted.secrets[1].1, "another-client-secret").unwrap();
        let error = prepare_in(
            &fixture.paths(),
            &fixture.wanted,
            &fixture.seed,
            None,
            false,
        )
        .unwrap_err();
        assert!(
            format!("{error:#}").contains("VEOVEO_IDP_OIDC_CLIENT_SECRET differs"),
            "{error:#}"
        );
        assert!(!format!("{error:#}").contains("another-client-secret"));
        assert_eq!(fixture.files(), before);
        let moved = Wanted {
            issuer: "https://other.example.com".into(),
            config: fixture.wanted.config.clone(),
            environment: fixture.wanted.environment.clone(),
            secrets: Vec::new(),
        };
        let error = prepare_in(&fixture.paths(), &moved, &fixture.seed, None, false).unwrap_err();
        assert!(
            format!("{error:#}").contains("origin does not change"),
            "{error:#}"
        );
        assert_eq!(fixture.files(), before);
    }

    #[test]
    fn incomplete_retained_state_never_creates_replacement_keys() {
        let fixture = Fixture::new("incomplete");
        let paths = fixture.paths();
        private_directory(&paths.directory).unwrap();
        std::fs::write(&paths.seed, "{}").unwrap();
        let error = prepare_in(&paths, &fixture.wanted, &fixture.seed, None, false).unwrap_err();
        assert!(error.to_string().contains("recover hub.env"), "{error}");
        assert_eq!(fixture.files().len(), 1);
        std::fs::remove_file(&paths.seed).unwrap();
        std::fs::write(
            &paths.environment,
            "VEOVEO_ISSUER=https://veoveo.example.com\n",
        )
        .unwrap();
        let error = prepare_in(&paths, &fixture.wanted, &fixture.seed, None, false).unwrap_err();
        assert!(
            format!("{error:#}").contains("never made again"),
            "{error:#}"
        );
        assert_eq!(
            fixture.files(),
            [(
                "hub.env".to_owned(),
                b"VEOVEO_ISSUER=https://veoveo.example.com\n".to_vec()
            )]
        );
    }

    #[test]
    fn a_check_writes_nothing() {
        let fixture = Fixture::new("check");
        let report =
            prepare_in(&fixture.paths(), &fixture.wanted, &fixture.seed, None, true).unwrap();
        assert!(
            report.lines.iter().all(|line| line.contains("would be")),
            "{report:?}"
        );
        assert!(fixture.files().is_empty());
    }

    #[test]
    fn a_seed_keeps_the_earlier_seeds_people() {
        let earlier = json!({
            "authorization": {"grants": [{"role": "admin", "subject": {"principal": "e8c1"}}]},
            "workContexts": [{"id": "reference-work", "memberships": [{"principals": ["e8c1"]}]}],
            "lore": {"repositories": [{"id": "01a1"}], "assignments": [{"repository": "01a1", "principal": "e8c1"}]},
        });
        let fixture = Fixture::new("people");
        let document = seed_document(&fixture.seed, Some(&earlier), "kid-1");
        assert_eq!(
            document["authorization"]["grants"],
            earlier["authorization"]["grants"]
        );
        assert_eq!(
            document["workContexts"][0]["memberships"],
            earlier["workContexts"][0]["memberships"]
        );
        assert_eq!(
            document["lore"]["repositories"],
            earlier["lore"]["repositories"]
        );
        assert_eq!(
            document["authorizationServers"][0]["accessTokenKeyId"],
            "kid-1"
        );
    }
}
