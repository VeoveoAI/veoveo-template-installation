//! The Mac development fixtures of local Keycloak: the persistent Keycloak itself, the
//! OpenShell management clients of the native gateway and Hub's local service and
//! public clients. Their private state stays in this repository's ignored `state/` and
//! `tls/`. Keycloak is changed only through its administration REST API, as the
//! permanent local administrator; no response body is printed, since one can hold a
//! secret.

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result, anyhow, bail, ensure};
use reqwest::Method;
use serde_json::{Value, json};

use crate::identity::files::{
    base64, ed25519_key, parse_environment, private_directory, random_bytes, render_environment,
    token_urlsafe, write_private,
};

pub const ISSUER: &str = "https://auth.localhost:8443/realms/veoveo";
const BASE: &str = "https://auth.localhost:8443";
const JWKS: &str =
    "https://host.openshell.internal:8443/realms/veoveo/protocol/openid-connect/certs";
/// The native Hub exposure of the Mac fixture.
pub const HUB_ORIGIN: &str = "https://vv-control--hub--http.openshell.localhost:17670";
const REALM: &str = "/admin/realms/veoveo";
const ROLES: [&str; 3] = ["member", "admin", "forwarder"];

/// A Keycloak administration API: one request, its JSON answer or `null`.
pub(crate) trait Admin {
    async fn call(&self, method: Method, path: &str, body: Option<Value>) -> Result<Value>;

    async fn get(&self, path: &str) -> Result<Value> {
        self.call(Method::GET, path, None).await
    }
}

/// The local Keycloak, reached at its loopback HTTPS listener with the installation CA.
pub struct Keycloak {
    http: reqwest::Client,
    token: String,
}

fn http(repository: &Path) -> Result<reqwest::Client> {
    let ca = repository.join("tls/ca/ca.crt");
    let pem = std::fs::read(&ca)
        .with_context(|| format!("cannot read the installation CA {}", ca.display()))?;
    let certificate =
        reqwest::Certificate::from_pem(&pem).context("the installation CA is not PEM")?;
    Ok(reqwest::Client::builder()
        .add_root_certificate(certificate)
        .resolve("auth.localhost", "127.0.0.1:8443".parse()?)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(15))
        .build()?)
}

async fn token(http: &reqwest::Client, form: &[(&str, &str)]) -> Result<String> {
    let response = http
        .post(format!(
            "{BASE}/realms/master/protocol/openid-connect/token"
        ))
        .form(form)
        .send()
        .await
        .context("cannot reach local Keycloak")?;
    let status = response.status();
    ensure!(
        status.is_success(),
        "Keycloak refused the administrator sign-in (HTTP {status})"
    );
    let body: Value = response.json().await.context("Keycloak answered no JSON")?;
    body["access_token"]
        .as_str()
        .map(str::to_owned)
        .context("Keycloak answered no access token")
}

impl Keycloak {
    /// Signs in as the permanent local administrator `credentials.json` names.
    pub async fn administrator(repository: &Path) -> Result<Self> {
        let credentials = Credentials::read(repository)?;
        let http = http(repository)?;
        let token = token(
            &http,
            &[
                ("grant_type", "password"),
                ("client_id", "admin-cli"),
                ("username", credentials.text("adminUsername")?),
                ("password", credentials.text("adminPassword")?),
            ],
        )
        .await?;
        Ok(Self { http, token })
    }
}

impl Admin for Keycloak {
    async fn call(&self, method: Method, path: &str, body: Option<Value>) -> Result<Value> {
        let mut request = self
            .http
            .request(method.clone(), format!("{BASE}{path}"))
            .bearer_auth(&self.token);
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request
            .send()
            .await
            .with_context(|| format!("cannot reach Keycloak for {method} {path}"))?;
        let status = response.status();
        ensure!(
            status.is_success(),
            "Keycloak answered HTTP {status} for {method} {}",
            path.split('?').next().unwrap_or(path)
        );
        let bytes = response.bytes().await?;
        if bytes.is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_slice(&bytes)
            .with_context(|| format!("Keycloak answered no JSON for {method} {path}"))
    }
}

/// `state/identity/credentials.json`: the local accounts and database passwords.
pub struct Credentials(Value);

impl Credentials {
    fn path(repository: &Path) -> PathBuf {
        repository.join("state/identity/credentials.json")
    }

    pub fn read(repository: &Path) -> Result<Self> {
        let path = Self::path(repository);
        let text = std::fs::read_to_string(&path).with_context(|| {
            format!(
                "cannot read {}; provision local Keycloak first",
                path.display()
            )
        })?;
        Ok(Self(serde_json::from_str(&text).with_context(|| {
            format!("{} is not JSON", path.display())
        })?))
    }

    fn text(&self, name: &str) -> Result<&str> {
        self.0[name]
            .as_str()
            .with_context(|| format!("credentials.json holds no {name}"))
    }
}

/// The ID of the entry named `name` in a list Keycloak answered.
fn id_named(list: &Value, member: &str, name: &str) -> Result<String> {
    list.as_array()
        .into_iter()
        .flatten()
        .find(|item| item[member] == name)
        .and_then(|item| item["id"].as_str())
        .map(str::to_owned)
        .with_context(|| format!("Keycloak has no {name}"))
}

/// A hard-coded claim mapper in access tokens only.
pub fn claim(name: &str, value: &str) -> Value {
    json!({"name": name, "protocol": "openid-connect", "protocolMapper": "oidc-hardcoded-claim-mapper",
        "config": {"claim.name": name.replace('.', r"\."), "claim.value": value, "jsonType.label": "String",
            "access.token.claim": "true", "id.token.claim": "false", "userinfo.token.claim": "false"}})
}

fn audience(audience: &str) -> Value {
    json!({"name": "audience", "protocol": "openid-connect", "protocolMapper": "oidc-audience-mapper",
        "config": {"included.custom.audience": audience, "access.token.claim": "true",
            "id.token.claim": "false", "userinfo.token.claim": "false"}})
}

/// Makes a client's default and optional scopes exactly `default` and `optional`.
async fn set_scopes(
    admin: &impl Admin,
    client: &str,
    default: &[String],
    optional: &[String],
) -> Result<()> {
    for (kind, wanted) in [("default", default), ("optional", optional)] {
        let link = format!("{REALM}/clients/{client}/{kind}-client-scopes");
        let present: Vec<String> = admin
            .get(&link)
            .await?
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|scope| scope["id"].as_str().map(str::to_owned))
            .collect();
        for scope in present.iter().filter(|scope| !wanted.contains(scope)) {
            admin
                .call(Method::DELETE, &format!("{link}/{scope}"), None)
                .await?;
        }
        for scope in wanted.iter().filter(|scope| !present.contains(scope)) {
            admin
                .call(Method::PUT, &format!("{link}/{scope}"), None)
                .await?;
        }
    }
    Ok(())
}

/// Makes a client's protocol mappers exactly `mappers`, by name.
async fn set_mappers(admin: &impl Admin, client: &str, mappers: &[Value]) -> Result<()> {
    let path = format!("{REALM}/clients/{client}/protocol-mappers/models");
    let current = admin.get(&path).await?;
    let current = current.as_array().cloned().unwrap_or_default();
    for mapper in &current {
        if !mappers
            .iter()
            .any(|wanted| wanted["name"] == mapper["name"])
        {
            let id = mapper["id"].as_str().context("a mapper has no ID")?;
            admin
                .call(Method::DELETE, &format!("{path}/{id}"), None)
                .await?;
        }
    }
    for mapper in mappers {
        match current
            .iter()
            .find(|existing| existing["name"] == mapper["name"])
        {
            Some(existing) => {
                let id = existing["id"].as_str().context("a mapper has no ID")?;
                admin
                    .call(Method::PUT, &format!("{path}/{id}"), Some(mapper.clone()))
                    .await?;
            }
            None => {
                admin
                    .call(Method::POST, &path, Some(mapper.clone()))
                    .await?;
            }
        }
    }
    Ok(())
}

/// The Keycloak ID of client `client_id`, creating it from `representation` or
/// updating it.
async fn put_client(admin: &impl Admin, client_id: &str, representation: &Value) -> Result<String> {
    let lookup = format!("{REALM}/clients?clientId={client_id}");
    let matches = admin.get(&lookup).await?;
    match matches.as_array().map(Vec::as_slice) {
        Some([existing]) => {
            let key = existing["id"]
                .as_str()
                .context("a client has no ID")?
                .to_owned();
            admin
                .call(
                    Method::PUT,
                    &format!("{REALM}/clients/{key}"),
                    Some(representation.clone()),
                )
                .await?;
            Ok(key)
        }
        Some([]) | None => {
            admin
                .call(
                    Method::POST,
                    &format!("{REALM}/clients"),
                    Some(representation.clone()),
                )
                .await?;
            let created = admin.get(&lookup).await?;
            created[0]["id"]
                .as_str()
                .map(str::to_owned)
                .context("Keycloak did not create the client")
        }
        Some(_) => bail!("Keycloak holds more than one client {client_id}"),
    }
}

// ----- local Keycloak -----

/// Provisions the persistent local Keycloak: its credentials, database, TLS leaf,
/// Compose services, permanent administrator and the `veoveo` realm. A credential is
/// made once; one that exists is never replaced.
pub async fn provision(repository: &Path, renew_certificate: bool) -> Result<()> {
    if renew_certificate {
        let backup = certificate(repository, true)?;
        println!(
            "Renewed the identity leaf under the existing CA and key; the earlier leaf is in {}.",
            backup
                .map(|path| path.display().to_string())
                .unwrap_or_default()
        );
        println!(
            "No service or identity record changed. Reload only the identity HTTPS listener to take it."
        );
        return Ok(());
    }
    let state = repository.join("state/identity");
    private_directory(&state)?;
    let path = Credentials::path(repository);
    let mut config: Value = if path.exists() {
        serde_json::from_str(&std::fs::read_to_string(&path)?)
            .with_context(|| format!("{} is not JSON", path.display()))?
    } else {
        let present = std::process::Command::new("docker")
            .args(["volume", "inspect", "vv-local_vv-keycloak-db"])
            .output()
            .context("cannot run docker")?
            .status
            .success();
        ensure!(
            !present,
            "recover the identity credentials for the existing Keycloak database; they are never replaced"
        );
        let user = std::env::var("USER").context("USER names no account")?;
        let mut config = json!({
            "username": user,
            "userId": uuid::Uuid::new_v4().to_string(),
            "adminUsername": format!("{user}-admin"),
            "adminId": uuid::Uuid::new_v4().to_string(),
        });
        for key in [
            "password",
            "adminPassword",
            "databasePassword",
            "postgresPassword",
            "bootstrapSecret",
        ] {
            config[key] = Value::String(token_urlsafe(36)?);
        }
        write_private(
            &path,
            &(serde_json::to_string_pretty(&config)? + "\n"),
            true,
        )?;
        config
    };
    let text = |name: &str| -> Result<String> {
        config[name]
            .as_str()
            .map(str::to_owned)
            .with_context(|| format!("credentials.json holds no {name}"))
    };
    let database = text("databasePassword")?;
    write_private(
        &state.join("database.env"),
        &format!(
            "POSTGRES_PASSWORD={}\nKEYCLOAK_DATABASE_PASSWORD={database}\n",
            text("postgresPassword")?
        ),
        false,
    )?;
    let mut keycloak = format!("KC_DB_PASSWORD={database}\n");
    let bootstrap = config["bootstrapSecret"].as_str().map(str::to_owned);
    if let Some(secret) = &bootstrap {
        keycloak.push_str(&format!("KC_BOOTSTRAP_ADMIN_CLIENT_ID=veoveo-bootstrap\nKC_BOOTSTRAP_ADMIN_CLIENT_SECRET={secret}\n"));
    }
    write_private(&state.join("keycloak.env"), &keycloak, false)?;
    certificate(repository, false)?;
    enable_profile(repository)?;
    docker(repository, &["volume", "create", "vv-local_vv-keycloak-db"])?;
    docker(repository, &["compose", "config", "--quiet"])?;
    println!("Starting persistent Keycloak, its PostgreSQL and the TLS edge.");
    docker(
        repository,
        &[
            "compose",
            "up",
            "-d",
            "vv-keycloak-db",
            "vv-keycloak",
            "vv-identity-edge",
        ],
    )?;
    let http = http(repository)?;
    let mut ready = false;
    for _ in 0..90 {
        if http
            .get(format!(
                "{BASE}/realms/master/.well-known/openid-configuration"
            ))
            .send()
            .await
            .is_ok_and(|response| response.status().is_success())
        {
            ready = true;
            break;
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    ensure!(ready, "Keycloak did not become ready");
    if let Some(secret) = bootstrap {
        let admin = Keycloak {
            token: token(
                &http,
                &[
                    ("grant_type", "client_credentials"),
                    ("client_id", "veoveo-bootstrap"),
                    ("client_secret", &secret),
                ],
            )
            .await?,
            http: http.clone(),
        };
        let admin_id = bootstrap_realm(&admin, &config).await?;
        config["adminId"] = Value::String(admin_id);
        config
            .as_object_mut()
            .context("credentials.json is an object")?
            .remove("bootstrapSecret");
        write_private(
            &path,
            &(serde_json::to_string_pretty(&config)? + "\n"),
            false,
        )?;
        write_private(
            &state.join("keycloak.env"),
            &format!("KC_DB_PASSWORD={database}\n"),
            false,
        )?;
        // The bootstrap credential leaves Docker's container configuration too.
        docker(
            repository,
            &["compose", "up", "-d", "--force-recreate", "vv-keycloak"],
        )?;
    }
    println!("Persistent OIDC issuer: {ISSUER}");
    println!("Initial account details: state/identity/credentials.json (owner-only, ignored).");
    println!("The temporary bootstrap administrator has been removed.");
    Ok(())
}

/// With the bootstrap client: the permanent administrator, the `veoveo` realm and its
/// user, then the bootstrap client removed. Answers the administrator's ID.
pub(crate) async fn bootstrap_realm(admin: &impl Admin, config: &Value) -> Result<String> {
    let text = |name: &str| -> Result<&str> {
        config[name]
            .as_str()
            .with_context(|| format!("credentials.json holds no {name}"))
    };
    let username = text("adminUsername")?;
    let users = admin
        .get(&format!(
            "/admin/realms/master/users?username={username}&exact=true"
        ))
        .await?;
    let admin_id = match users.as_array().and_then(|users| users.first()) {
        Some(user) => user["id"].as_str().context("a user has no ID")?.to_owned(),
        None => {
            admin
                .call(Method::POST, "/admin/realms/master/users", Some(json!({
                    "id": text("adminId")?, "username": username, "enabled": true,
                    "credentials": [{"type": "password", "value": text("adminPassword")?, "temporary": true}],
                })))
                .await?;
            text("adminId")?.to_owned()
        }
    };
    let role = admin.get("/admin/realms/master/roles/admin").await?;
    admin
        .call(
            Method::POST,
            &format!("/admin/realms/master/users/{admin_id}/role-mappings/realm"),
            Some(json!([role])),
        )
        .await?;
    let realms = admin.get("/admin/realms").await?;
    if !realms
        .as_array()
        .into_iter()
        .flatten()
        .any(|realm| realm["realm"] == "veoveo")
    {
        admin
            .call(Method::POST, "/admin/realms", Some(json!({
                "realm": "veoveo", "enabled": true, "displayName": "Veoveo Local", "sslRequired": "all",
                "registrationAllowed": false, "bruteForceProtected": true, "resetPasswordAllowed": false,
                "rememberMe": true, "accessTokenLifespan": 300, "ssoSessionIdleTimeout": 1800,
                "ssoSessionMaxLifespan": 36000,
                // An imported user gets no default role by itself; the account console needs them.
                "users": [{"id": text("userId")?, "username": text("username")?, "enabled": true,
                    "realmRoles": ["default-roles-veoveo"],
                    "credentials": [{"type": "password", "value": text("password")?, "temporary": false}]}],
            })))
            .await?;
    }
    let mut profile = admin.get(&format!("{REALM}/users/profile")).await?;
    if let Some(attributes) = profile["attributes"].as_array_mut() {
        for attribute in attributes
            .iter_mut()
            .filter(|attribute| attribute["name"] != "username")
        {
            if let Some(attribute) = attribute.as_object_mut() {
                attribute.remove("required");
            }
        }
    }
    admin
        .call(
            Method::PUT,
            &format!("{REALM}/users/profile"),
            Some(profile),
        )
        .await?;
    let clients = admin
        .get("/admin/realms/master/clients?clientId=veoveo-bootstrap")
        .await?;
    for client in clients.as_array().into_iter().flatten() {
        let id = client["id"].as_str().context("a client has no ID")?;
        admin
            .call(
                Method::DELETE,
                &format!("/admin/realms/master/clients/{id}"),
                None,
            )
            .await?;
    }
    Ok(admin_id)
}

fn docker(repository: &Path, arguments: &[&str]) -> Result<()> {
    let output = std::process::Command::new("docker")
        .args(arguments)
        .current_dir(repository)
        .output()
        .context("cannot run docker")?;
    // Its output can hold a secret; only the command is named.
    ensure!(
        output.status.success(),
        "docker {} failed",
        arguments.join(" ")
    );
    Ok(())
}

fn openssl(repository: &Path, arguments: &[&str]) -> Result<Vec<u8>> {
    let output = std::process::Command::new("openssl")
        .args(arguments)
        .current_dir(repository)
        .output()
        .context("cannot run openssl")?;
    ensure!(
        output.status.success(),
        "openssl {} failed",
        arguments.first().copied().unwrap_or_default()
    );
    Ok(output.stdout)
}

/// `COMPOSE_PROFILES` in the ignored `.env` includes `keycloak`.
fn enable_profile(repository: &Path) -> Result<()> {
    let path = repository.join(".env");
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("cannot read {}", path.display()))?;
    let mut profiles: Vec<String> = text
        .lines()
        .find_map(|line| line.strip_prefix("COMPOSE_PROFILES="))
        .map(|value| {
            value
                .split(',')
                .filter(|profile| !profile.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    if !profiles.iter().any(|profile| profile == "keycloak") {
        profiles.push("keycloak".to_owned());
    }
    let mut lines: Vec<&str> = text
        .lines()
        .filter(|line| !line.starts_with("COMPOSE_PROFILES="))
        .collect();
    let setting = format!("COMPOSE_PROFILES={}", profiles.join(","));
    lines.push(&setting);
    write_private(&path, &(lines.join("\n") + "\n"), false)
}

/// The identity edge's TLS leaf under the installation CA: issued once, or renewed under
/// its existing key with the earlier leaf kept privately.
fn certificate(repository: &Path, renew: bool) -> Result<Option<PathBuf>> {
    let directory = repository.join("tls/identity");
    std::fs::create_dir_all(&directory)?;
    let (key, cert) = (
        directory.join("identity.key"),
        directory.join("identity.crt"),
    );
    ensure!(
        key.exists() == cert.exists(),
        "the identity TLS material is incomplete"
    );
    let existing = key.exists();
    if existing && !renew {
        return Ok(None);
    }
    ensure!(
        !renew || existing,
        "renewal needs the existing identity leaf"
    );
    let (ca, ca_key) = (
        repository.join("tls/ca/ca.crt"),
        repository.join("tls/private/ca.key"),
    );
    ensure!(
        ca.is_file() && ca_key.is_file(),
        "keep and restore the existing installation CA before issuing a leaf"
    );
    let stage = repository
        .join("tls/private")
        .join(format!(".identity-{}", std::process::id()));
    private_directory(&stage)?;
    let result = (|| -> Result<Option<PathBuf>> {
        let path = |name: &str| stage.join(name).display().to_string();
        std::fs::write(
            stage.join("extensions"),
            "subjectAltName=DNS:auth.localhost,DNS:host.openshell.internal,IP:127.0.0.1\nbasicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature\nextendedKeyUsage=serverAuth\nsubjectKeyIdentifier=hash\nauthorityKeyIdentifier=keyid,issuer\n",
        )?;
        let (key_text, cert_text) = (key.display().to_string(), cert.display().to_string());
        if existing {
            // Renew under the same key, so no moment pairs a new leaf with the old key.
            let public = openssl(repository, &["pkey", "-in", &key_text, "-pubout"])?;
            ensure!(
                public
                    == openssl(
                        repository,
                        &["x509", "-in", &cert_text, "-pubkey", "-noout"]
                    )?,
                "the identity certificate and key differ"
            );
            openssl(
                repository,
                &[
                    "req",
                    "-new",
                    "-key",
                    &key_text,
                    "-subj",
                    "/CN=auth.localhost",
                    "-out",
                    &path("request"),
                ],
            )?;
        } else {
            openssl(
                repository,
                &[
                    "req",
                    "-newkey",
                    "ec",
                    "-pkeyopt",
                    "ec_paramgen_curve:P-256",
                    "-nodes",
                    "-subj",
                    "/CN=auth.localhost",
                    "-keyout",
                    &path("key"),
                    "-out",
                    &path("request"),
                ],
            )?;
        }
        let serial = format!(
            "0x{}",
            random_bytes(19)?
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        );
        let (ca_text, ca_key_text) = (ca.display().to_string(), ca_key.display().to_string());
        openssl(
            repository,
            &[
                "x509",
                "-req",
                "-in",
                &path("request"),
                "-CA",
                &ca_text,
                "-CAkey",
                &ca_key_text,
                "-set_serial",
                &serial,
                "-days",
                "365",
                "-extfile",
                &path("extensions"),
                "-out",
                &path("certificate"),
            ],
        )?;
        openssl(
            repository,
            &["verify", "-CAfile", &ca_text, &path("certificate")],
        )?;
        for host in ["auth.localhost", "host.openshell.internal"] {
            openssl(
                repository,
                &[
                    "x509",
                    "-in",
                    &path("certificate"),
                    "-noout",
                    "-checkhost",
                    host,
                ],
            )?;
        }
        openssl(
            repository,
            &[
                "x509",
                "-in",
                &path("certificate"),
                "-noout",
                "-checkip",
                "127.0.0.1",
            ],
        )?;
        let mut backup = None;
        if existing {
            let backups = repository.join("state/identity/tls-backups");
            private_directory(&backups)?;
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs();
            let folder = backups.join(format!(
                "{stamp}-{}",
                random_bytes(4)?
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()
            ));
            private_directory(&folder)?;
            for source in [&key, &cert] {
                let destination = folder.join(source.file_name().context("a TLS file has a name")?);
                std::fs::copy(source, &destination)?;
                write_private(&destination, &std::fs::read_to_string(&destination)?, false)?;
            }
            backup = Some(folder);
        } else {
            std::fs::rename(stage.join("key"), &key)?;
        }
        std::fs::rename(stage.join("certificate"), &cert)?;
        Ok(backup)
    })();
    let _ = std::fs::remove_dir_all(&stage);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        if key.exists() {
            std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600))?;
        }
        if cert.exists() {
            std::fs::set_permissions(&cert, std::fs::Permissions::from_mode(0o644))?;
        }
    }
    result
}

// ----- OpenShell management clients -----

/// The native gateway's local OIDC clients: the public CLI client, the controller and a
/// nonmember for acceptance, with their secrets made once in `state/openshell`.
pub async fn configure_openshell(repository: &Path) -> Result<()> {
    let admin = Keycloak::administrator(repository).await?;
    let credentials = Credentials::read(repository)?;
    configure_openshell_with(&admin, repository, credentials.text("userId")?).await?;
    println!(
        "Configured local OpenShell OIDC clients; the controller and nonmember credentials stay private."
    );
    Ok(())
}

pub(crate) async fn configure_openshell_with(
    admin: &impl Admin,
    repository: &Path,
    user_id: &str,
) -> Result<()> {
    let directory = repository.join("state/openshell");
    private_directory(&directory)?;
    let controller = directory.join("controller.secret");
    let outsider = directory.join("outsider.secret");
    for path in [&controller, &outsider] {
        if !path.exists() {
            write_private(path, &token_urlsafe(36)?, true)?;
        }
    }
    for role in ["openshell-admin", "openshell-user"] {
        let roles = admin.get(&format!("{REALM}/roles")).await?;
        if !roles
            .as_array()
            .into_iter()
            .flatten()
            .any(|existing| existing["name"] == role)
        {
            admin
                .call(
                    Method::POST,
                    &format!("{REALM}/roles"),
                    Some(json!({"name": role})),
                )
                .await?;
        }
    }
    let admin_role = admin.get(&format!("{REALM}/roles/openshell-admin")).await?;
    for (client_id, public) in [
        ("openshell-cli", true),
        ("veoveo-controller", false),
        ("veoveo-outsider", false),
    ] {
        let mut representation = openshell_client(client_id, public);
        if !public {
            let secret = if client_id == "veoveo-outsider" {
                &outsider
            } else {
                &controller
            };
            representation["secret"] =
                Value::String(std::fs::read_to_string(secret)?.trim().to_owned());
        }
        let key = put_client(admin, client_id, &representation).await?;
        let scopes = admin.get(&format!("{REALM}/client-scopes")).await?;
        for name in ["basic", "profile", "roles"] {
            let scope = id_named(&scopes, "name", name)?;
            admin
                .call(
                    Method::PUT,
                    &format!("{REALM}/clients/{key}/default-client-scopes/{scope}"),
                    None,
                )
                .await?;
        }
        if public {
            // Updating a client does not update the scope links it already has.
            let offline = id_named(&scopes, "name", "offline_access")?;
            admin
                .call(
                    Method::PUT,
                    &format!("{REALM}/clients/{key}/optional-client-scopes/{offline}"),
                    None,
                )
                .await?;
        }
        let subject = if public {
            user_id.to_owned()
        } else {
            admin
                .get(&format!("{REALM}/clients/{key}/service-account-user"))
                .await?["id"]
                .as_str()
                .context("a service account has no ID")?
                .to_owned()
        };
        let role = if client_id == "veoveo-outsider" {
            admin.get(&format!("{REALM}/roles/openshell-user")).await?
        } else {
            admin_role.clone()
        };
        admin
            .call(
                Method::POST,
                &format!("{REALM}/users/{subject}/role-mappings/realm"),
                Some(json!([role])),
            )
            .await?;
    }
    let mut config = json!({"issuer": ISSUER, "clientId": "veoveo-controller",
        "issuerCaFile": repository.join("tls/ca/ca.crt").display().to_string(),
        "clientSecretFile": controller.display().to_string(), "audience": "openshell",
        "gateway": "https://localhost:17670", "gatewayCaFile": "/opt/homebrew/var/openshell/tls/ca.crt"});
    write_private(
        &directory.join("controller.json"),
        &(serde_json::to_string_pretty(&config)? + "\n"),
        false,
    )?;
    config["clientId"] = json!("veoveo-outsider");
    config["clientSecretFile"] = json!(outsider.display().to_string());
    write_private(
        &directory.join("outsider.json"),
        &(serde_json::to_string_pretty(&config)? + "\n"),
        false,
    )
}

fn openshell_client(client_id: &str, public: bool) -> Value {
    json!({
        "clientId": client_id, "protocol": "openid-connect", "enabled": true,
        "publicClient": public, "standardFlowEnabled": public, "serviceAccountsEnabled": !public,
        "directAccessGrantsEnabled": false, "implicitFlowEnabled": false,
        "defaultClientScopes": ["basic", "profile", "roles"],
        "optionalClientScopes": if public { json!(["offline_access"]) } else { json!([]) },
        // Keycloak matches HTTP loopback redirects on any port (RFC 8252); the host and
        // path stay exact.
        "redirectUris": if public { json!(["http://127.0.0.1/callback", "http://127.0.0.1:80/callback"]) } else { json!([]) },
        "attributes": {"access.token.lifespan": "120", "pkce.code.challenge.method": "S256"},
        "protocolMappers": [
            {"name": "openshell-audience", "protocol": "openid-connect", "protocolMapper": "oidc-audience-mapper",
             "config": {"included.custom.audience": "openshell", "access.token.claim": "true"}},
            {"name": "openshell-scopes", "protocol": "openid-connect", "protocolMapper": "oidc-hardcoded-claim-mapper",
             "config": {"claim.name": "openshell_scopes", "claim.value": "openshell:all", "jsonType.label": "String", "access.token.claim": "true"}},
        ],
    })
}

// ----- Hub's local fixture -----

/// The private native bindings of the Mac fixture's servers.
pub fn binding(server: &str) -> Value {
    let port = match server {
        "time" => 8800,
        "artifact" => 8801,
        "media" => 8802,
        _ => 8803,
    };
    json!({"endpoint": format!("http://{server}.internal:{port}/{server}/mcp"), "workspace": "vv-services", "sandbox": server, "port": port})
}

/// Which servers a fixture catalog binds: Time always, and Artifact, Media and Embedding
/// when selected now or before. A catalog bound otherwise is refused.
pub fn select_services(
    artifact: bool,
    media: bool,
    embedding: bool,
    retained: Option<&Value>,
) -> Result<(bool, bool, bool)> {
    let (mut artifact, mut media, mut embedding) = (artifact, media, embedding);
    if let Some(retained) = retained {
        let bindings = retained["nativeUpstreams"]["bindings"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let approved: Vec<Vec<Value>> = [
            vec!["time"],
            vec!["time", "artifact"],
            vec!["time", "artifact", "media"],
        ]
        .into_iter()
        .flat_map(|servers| {
            let mut with = servers.clone();
            with.push("embedding");
            [servers, with]
        })
        .map(|servers| servers.into_iter().map(binding).collect())
        .collect();
        ensure!(
            approved.contains(&bindings),
            "the existing native bindings differ; review the operator's placement before preparing again"
        );
        artifact |= bindings.contains(&binding("artifact"));
        media |= bindings.contains(&binding("media"));
        embedding |= bindings.contains(&binding("embedding"));
    }
    ensure!(
        !media || artifact,
        "Media needs Artifact; select it or keep its approved binding"
    );
    Ok((artifact, media, embedding))
}

/// The Mac fixture's Hub catalog: local Keycloak as identity provider and authorization
/// server, and the service clients of each role. A retained catalog keeps its Work
/// Contexts and policies.
pub fn local_catalog(
    origin: &str,
    artifact: bool,
    media: bool,
    embedding: bool,
    retained: Option<&Value>,
) -> Result<Value> {
    let (artifact, media, embedding) = select_services(artifact, media, embedding, retained)?;
    let resource = format!("{origin}/mcp/local");
    let mut roles = vec!["member", "admin"];
    if media {
        roles.push("billed");
    }
    let clients: Vec<String> = roles.iter().map(|role| format!("vv-hub-{role}")).collect();
    let mut bindings = vec![binding("time")];
    if artifact {
        bindings.push(binding("artifact"));
    }
    if media {
        bindings.push(binding("media"));
    }
    if embedding {
        bindings.push(binding("embedding"));
    }
    let mut authorization_roles = vec![
        json!({"name": "member", "access": ["read", "write"]}),
        json!({"name": "admin", "access": ["read", "write"], "risks": ["sensitive", "destructive", "billed", "physical", "administer"]}),
    ];
    if media {
        authorization_roles
            .push(json!({"name": "billed", "access": ["read", "write"], "risks": ["billed"]}));
    }
    let token_endpoint = format!("{ISSUER}/protocol/openid-connect/token");
    let mut catalog = json!({
        "identityProviders": [{"id": "local", "issuer": ISSUER, "jwks": {"source": "remote", "jwksUri": JWKS},
            "trustedCertificateAuthorities": [{"source": "file", "path": "/etc/veoveo/hub/installation-ca.crt"}],
            "tokenEndpoint": token_endpoint, "metadata": {}}],
        "authorizationServers": [{"id": "local", "issuer": ISSUER, "identityProvider": "local",
            "jwks": {"source": "remote", "jwksUri": JWKS}, "tokenEndpoint": token_endpoint, "metadata": {}}],
        "profiles": [{"id": "local", "identityProvider": "local", "authorizationServer": "local",
            "protectedResource": resource, "policyVersion": "local", "authModes": ["oauth_client_credentials"],
            "requiredScopes": ["mcp"], "metadata": {}}],
        "authorization": {"roles": authorization_roles,
            "grants": roles.iter().map(|role| json!({"role": role, "subject": {"principal": format!("{ISSUER}#vv-hub-{role}")}})).collect::<Vec<_>>()},
        "registration": {"secret": "registration_secret"},
        "tenants": [{"id": "local", "title": "Local installation"}],
        "workContexts": [{"id": "local", "tenant": "local", "title": "Local", "policyRevision": "local",
            "outputPolicy": {"owner": {"kind": "principal", "id": format!("{ISSUER}#vv-hub-admin")}},
            "memberships": [{"level": "contributor", "oauthClients": clients}]}],
        "policies": [{"version": "local", "rules": []}],
        "dataLabels": [],
        "oidcClients": [],
        "oauthClients": clients.iter().map(|client| json!({"id": client, "authorizationServer": "local",
            "defaultWorkContext": "local", "invocationMode": "automated", "allowedResources": [resource],
            "grantTypes": ["client_credentials"], "authMethods": ["client_secret_post"], "allowedScopes": ["mcp"],
            "tenant": "local", "metadata": {}})).collect::<Vec<_>>(),
        "secrets": [{"id": "registration_secret", "source": "env", "purpose": "registration_secret",
            "locator": "VEOVEO_REGISTRATION_SECRET", "owner": {"kind": "hub"}, "metadata": {}}],
        "nativeUpstreams": {"gateway": "https://host.openshell.internal:17670",
            "certificateAuthority": "/run/openshell-supervisor-ca/material/ca.crt",
            "bearerEnvironment": "HUB_MANAGEMENT_REFERENCE", "bindings": bindings},
        "metadata": {},
    });
    if let Some(retained) = retained {
        catalog["workContexts"] = retained["workContexts"].clone();
        catalog["policies"] = retained["policies"].clone();
        if media {
            let contexts = catalog["workContexts"]
                .as_array_mut()
                .context("a retained catalog has Work Contexts")?;
            let mut local: Vec<&mut Value> = contexts
                .iter_mut()
                .filter(|context| context["id"] == "local" && context["tenant"] == "local")
                .collect();
            ensure!(
                local.len() == 1,
                "review the retained local Work Context before admitting the billed client"
            );
            let memberships = local[0]["memberships"]
                .as_array_mut()
                .context("a Work Context has memberships")?;
            let mut contributors: Vec<&mut Value> = memberships
                .iter_mut()
                .filter(|membership| {
                    membership["level"] == "contributor"
                        && ["vv-hub-member", "vv-hub-admin"].iter().all(|client| {
                            membership["oauthClients"]
                                .as_array()
                                .is_some_and(|clients| clients.iter().any(|value| value == client))
                        })
                })
                .collect();
            ensure!(
                contributors.len() == 1,
                "review the retained contributor membership before admitting the billed client"
            );
            let clients = contributors[0]["oauthClients"]
                .as_array_mut()
                .context("a membership lists clients")?;
            if !clients.iter().any(|client| client == "vv-hub-billed") {
                clients.push(json!("vv-hub-billed"));
            }
        }
    }
    Ok(catalog)
}

/// The fixture Hub's `hub.env`: made once, then recovered; a missing key or another
/// origin is refused.
pub fn local_environment(path: &Path, origin: &str) -> Result<()> {
    let required = [
        "VEOVEO_FORWARDED_TOKEN_KEY_B64",
        "VEOVEO_AUDIT_SIGNING_KEY_B64",
        "VEOVEO_REFRESH_DELIVERY_KEY_B64",
        "VEOVEO_REGISTRATION_SECRET",
        "VEOVEO_DATABASE_PASSWORD",
    ];
    let supplied = std::env::var("VEOVEO_DATABASE_PASSWORD").ok();
    if path.exists() {
        let values = parse_environment(&std::fs::read_to_string(path)?)?;
        let value = |name: &str| {
            values
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.as_str())
        };
        ensure!(
            required
                .iter()
                .all(|name| value(name).is_some_and(|value| !value.is_empty()))
                && value("VEOVEO_ISSUER") == Some(origin),
            "recover the existing Hub state; missing keys are never made again and the origin does not change"
        );
        if let Some(supplied) = supplied {
            ensure!(
                value("VEOVEO_DATABASE_PASSWORD") == Some(supplied.as_str()),
                "the database credential differs; rotate it in a separate, reviewed step"
            );
        }
        return Ok(());
    }
    let password = supplied
        .filter(|password| !password.is_empty() && !password.contains(['\n', '\r']))
        .context("a first preparation needs VEOVEO_DATABASE_PASSWORD from the trusted bootstrap")?;
    let values = vec![
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
        ("VEOVEO_DATABASE_PASSWORD".to_owned(), password),
        (
            "VEOVEO_DATABASE_URL".to_owned(),
            "postgres://veoveo_owner@host.openshell.internal:5432/veoveo".to_owned(),
        ),
        (
            "VEOVEO_SERVER_DATABASE_URL".to_owned(),
            "postgres://host.openshell.internal:5432/veoveo".to_owned(),
        ),
        ("VEOVEO_ISSUER".to_owned(), origin.to_owned()),
        (
            "VEOVEO_CONFIG".to_owned(),
            "/etc/veoveo/hub/hub.json".to_owned(),
        ),
        (
            "VEOVEO_LISTEN_ADDRESS".to_owned(),
            "127.0.0.1:8080".to_owned(),
        ),
    ];
    write_private(path, &render_environment(&values), true)
}

/// The Mac fixture's Hub: its catalog, keys and local Keycloak service clients
/// (`vv-hub-member`, `-admin`, `-forwarder` and, with Media, `-billed`).
pub async fn configure_hub(
    repository: &Path,
    origin: &str,
    artifact: bool,
    media: bool,
    embedding: bool,
) -> Result<()> {
    ensure!(
        origin.trim_end_matches('/') == HUB_ORIGIN,
        "the Mac fixture uses the reviewed vv-control/hub/http origin {HUB_ORIGIN}"
    );
    let state = repository.join("state/openshell/hub");
    let catalog_path = state.join("hub.json");
    let retained = match std::fs::read_to_string(&catalog_path) {
        Ok(text) => Some(serde_json::from_str::<Value>(&text)?),
        Err(_) => None,
    };
    let catalog = local_catalog(HUB_ORIGIN, artifact, media, embedding, retained.as_ref())?;
    private_directory(&state)?;
    let ca = std::fs::read_to_string(repository.join("tls/ca/ca.crt"))
        .context("cannot read the installation CA")?;
    reqwest::Certificate::from_pem(ca.as_bytes()).context("the installation CA is not PEM")?;
    write_private(&state.join("installation-ca.crt"), &ca, false)?;
    local_environment(&state.join("hub.env"), HUB_ORIGIN)?;
    let admin = Keycloak::administrator(repository).await?;
    let media = catalog["authorization"]["roles"]
        .as_array()
        .is_some_and(|roles| roles.iter().any(|role| role["name"] == "billed"));
    let scopes = shared_scopes(&admin).await?;
    let roles = admin.get(&format!("{REALM}/roles")).await?;
    if !roles
        .as_array()
        .into_iter()
        .flatten()
        .any(|role| role["name"] == "openshell-user")
    {
        admin
            .call(
                Method::POST,
                &format!("{REALM}/roles"),
                Some(json!({"name": "openshell-user"})),
            )
            .await?;
    }
    let mut all: Vec<&str> = ROLES.to_vec();
    if media {
        all.push("billed");
    }
    for role in all {
        let scope = if role == "forwarder" {
            &scopes.1
        } else {
            &scopes.0
        };
        let subject = service_client(&admin, &state, role, scope).await?;
        if role == "forwarder" {
            println!("Forwarder subject for native vv-services user membership: {subject}");
        }
    }
    write_private(
        &catalog_path,
        &(serde_json::to_string_pretty(&catalog)? + "\n"),
        false,
    )?;
    println!(
        "Prepared local identities and owner-only state/openshell/hub files; no workloads or database state changed."
    );
    println!(
        "Verify the actual native exposure equals the Hub origin before application qualification."
    );
    Ok(())
}

/// The shared `mcp` and `sandbox:write` scopes, created when absent and refused when
/// they carry mappers or omit their token scope.
async fn shared_scopes(admin: &impl Admin) -> Result<(String, String)> {
    let mut ids = Vec::new();
    for name in ["mcp", "sandbox:write"] {
        let scopes = admin.get(&format!("{REALM}/client-scopes")).await?;
        let scope = match scopes
            .as_array()
            .into_iter()
            .flatten()
            .find(|scope| scope["name"] == name)
        {
            Some(scope) => scope.clone(),
            None => {
                admin
                    .call(Method::POST, &format!("{REALM}/client-scopes"), Some(json!({"name": name, "protocol": "openid-connect", "attributes": {"include.in.token.scope": "true"}})))
                    .await?;
                let scopes = admin.get(&format!("{REALM}/client-scopes")).await?;
                scopes
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|scope| scope["name"] == name)
                    .cloned()
                    .context("Keycloak did not create the scope")?
            }
        };
        let id = scope["id"]
            .as_str()
            .context("a scope has no ID")?
            .to_owned();
        let mappers = admin
            .get(&format!(
                "{REALM}/client-scopes/{id}/protocol-mappers/models"
            ))
            .await?;
        ensure!(
            mappers.as_array().is_none_or(Vec::is_empty),
            "the shared scope {name} has unreviewed mappers"
        );
        ensure!(
            scope["attributes"]["include.in.token.scope"] == "true",
            "the scope {name} omits its token scope"
        );
        ids.push(id);
    }
    Ok((ids[0].clone(), ids[1].clone()))
}

/// One of Hub's service clients: its secret made once and kept, its account's realm
/// roles and groups removed, its scopes and mappers exactly Hub's. Answers its
/// service account's subject.
pub(crate) async fn service_client(
    admin: &impl Admin,
    state: &Path,
    role: &str,
    scope: &str,
) -> Result<String> {
    let client_id = format!("vv-hub-{role}");
    let management = role == "forwarder";
    let path = state.join(format!("{role}.secret"));
    let lookup = admin
        .get(&format!("{REALM}/clients?clientId={client_id}"))
        .await?;
    let existing = lookup
        .as_array()
        .and_then(|clients| clients.first())
        .and_then(|client| client["id"].as_str())
        .map(str::to_owned);
    let mut previous_subject = None;
    let secret = match &existing {
        Some(key) => {
            previous_subject = admin
                .get(&format!("{REALM}/clients/{key}/service-account-user"))
                .await?["id"]
                .as_str()
                .map(str::to_owned);
            let secret = admin
                .get(&format!("{REALM}/clients/{key}/client-secret"))
                .await?["value"]
                .as_str()
                .context("Keycloak answered no client secret")?
                .to_owned();
            if path.exists() {
                ensure!(
                    std::fs::read_to_string(&path)?.trim() == secret,
                    "the retained secret of {client_id} differs; recover custody before changing the client"
                );
            }
            secret
        }
        None if path.exists() => std::fs::read_to_string(&path)?.trim().to_owned(),
        None => token_urlsafe(36)?,
    };
    ensure!(
        !secret.is_empty(),
        "the retained secret of {client_id} is empty"
    );
    if !path.exists() {
        write_private(&path, &format!("{secret}\n"), true)?;
    }
    let representation = json!({"clientId": client_id, "protocol": "openid-connect", "enabled": true,
        "publicClient": false, "clientAuthenticatorType": "client-secret", "secret": secret,
        "standardFlowEnabled": false, "serviceAccountsEnabled": true, "directAccessGrantsEnabled": false,
        "implicitFlowEnabled": false, "fullScopeAllowed": false, "redirectUris": [],
        "attributes": {"access.token.lifespan": "300", "access.token.header.type.rfc9068": "true", "client_credentials.use_refresh_token": "false"}});
    let key = put_client(admin, &client_id, &representation).await?;
    let account = admin
        .get(&format!("{REALM}/clients/{key}/service-account-user"))
        .await?["id"]
        .as_str()
        .context("a service account has no ID")?
        .to_owned();
    if let Some(previous) = previous_subject {
        ensure!(
            previous == account,
            "the service account of {client_id} changed unexpectedly"
        );
    }
    let scopes = admin.get(&format!("{REALM}/client-scopes")).await?;
    let mut default = vec![id_named(&scopes, "name", "basic")?];
    if management {
        default.push(id_named(&scopes, "name", "roles")?);
    }
    set_scopes(admin, &key, &default, &[scope.to_owned()]).await?;
    // A dedicated account inherits no realm role or group.
    let user_roles = format!("{REALM}/users/{account}/role-mappings/realm");
    let current = admin.get(&user_roles).await?;
    if current.as_array().is_some_and(|roles| !roles.is_empty()) {
        admin
            .call(Method::DELETE, &user_roles, Some(current))
            .await?;
    }
    for group in admin
        .get(&format!("{REALM}/users/{account}/groups"))
        .await?
        .as_array()
        .cloned()
        .unwrap_or_default()
    {
        let id = group["id"].as_str().context("a group has no ID")?;
        admin
            .call(
                Method::DELETE,
                &format!("{REALM}/users/{account}/groups/{id}"),
                None,
            )
            .await?;
    }
    let scope_roles = format!("{REALM}/clients/{key}/scope-mappings/realm");
    let current = admin.get(&scope_roles).await?;
    if current.as_array().is_some_and(|roles| !roles.is_empty()) {
        admin
            .call(Method::DELETE, &scope_roles, Some(current))
            .await?;
    }
    let mut mappers = if management {
        let role = admin.get(&format!("{REALM}/roles/openshell-user")).await?;
        ensure!(
            role["composite"] != true,
            "openshell-user must not inherit broader realm roles"
        );
        admin
            .call(Method::POST, &user_roles, Some(json!([role.clone()])))
            .await?;
        admin
            .call(Method::POST, &scope_roles, Some(json!([role])))
            .await?;
        vec![claim("openshell_scopes", "sandbox:write")]
    } else {
        vec![
            claim("sub", &client_id),
            claim("client_id", &client_id),
            claim(
                "https://veoveo.ai/claims/principal",
                &format!("{ISSUER}#{client_id}"),
            ),
            claim("https://veoveo.ai/claims/principal-kind", "service"),
            claim("https://veoveo.ai/claims/tenant", "local"),
            claim("https://veoveo.ai/claims/work-context", "local"),
            claim("https://veoveo.ai/claims/invocation-mode", "automated"),
        ]
    };
    mappers.push(audience(&if management {
        "openshell".to_owned()
    } else {
        format!("{HUB_ORIGIN}/mcp/local")
    }));
    set_mappers(admin, &key, &mappers).await?;
    let prefix = format!("VV_HUB_{}", role.to_uppercase());
    write_private(
        &state.join(format!("{role}.refresh.env")),
        &render_environment(&[
            (format!("{prefix}_CLIENT_ID"), client_id.clone()),
            (format!("{prefix}_CLIENT_SECRET"), secret),
        ]),
        false,
    )?;
    Ok(account)
}

// ----- the local public client -----

/// What a local public client is made for.
pub struct PublicClient {
    pub client_id: String,
    pub members: Vec<String>,
    pub context: String,
    pub resource: String,
    pub redirects: Vec<String>,
}

/// A catalog with a local PKCE client and its human members added, nothing else
/// rebuilt; an existing different registration is refused.
pub fn local_public_catalog(retained: &Value, request: &PublicClient) -> Result<Value> {
    ensure!(
        !request.client_id.is_empty()
            && request
                .client_id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c)),
        "a client ID is a nonempty ASCII name"
    );
    let mut members: Vec<String> = request
        .members
        .iter()
        .map(|member| {
            uuid::Uuid::parse_str(member)
                .map(|id| id.to_string())
                .map_err(|_| anyhow!("{member:?} is not a UUID"))
        })
        .collect::<Result<_>>()?;
    members.sort();
    members.dedup();
    let mut redirects = request.redirects.clone();
    redirects.dedup();
    ensure!(
        !members.is_empty() && !redirects.is_empty(),
        "a local public client needs explicit human IDs and loopback redirects"
    );
    for redirect in &redirects {
        let url = url::Url::parse(redirect).map_err(|_| anyhow!("{redirect:?} is not a URL"))?;
        ensure!(
            url.scheme() == "http"
                && url.host_str() == Some("127.0.0.1")
                && url.port().is_some()
                && url.fragment().is_none(),
            "native OpenCode needs an explicit HTTP 127.0.0.1 callback port"
        );
    }
    let resource = url::Url::parse(&request.resource)
        .map_err(|_| anyhow!("the protected resource is not a URL"))?;
    ensure!(
        resource.scheme() == "https",
        "the protected resource is an HTTPS endpoint"
    );
    let context = retained["workContexts"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|context| context["id"] == request.context.as_str())
        .cloned()
        .collect::<Vec<_>>();
    let profile = retained["profiles"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|profile| profile["protectedResource"] == request.resource.as_str())
        .cloned()
        .collect::<Vec<_>>();
    ensure!(
        context.len() == 1 && profile.len() == 1,
        "select one existing Work Context and MCP resource"
    );
    let (context, profile) = (&context[0], &profile[0]);
    let servers = retained["authorizationServers"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|server| {
            server["id"] == profile["authorizationServer"]
                && server["issuer"] == ISSUER
                && server
                    .get("accessTokenSigningKey")
                    .is_none_or(Value::is_null)
        });
    ensure!(
        servers.count() == 1,
        "a local public client needs the existing external Keycloak issuer"
    );
    ensure!(
        retained["authorization"]["roles"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|role| role["name"] == "member"),
        "the existing member authority is required"
    );
    let client = json!({"id": request.client_id, "authorizationServer": profile["authorizationServer"],
        "defaultWorkContext": request.context, "invocationMode": "direct", "allowedResources": [request.resource],
        "grantTypes": ["authorization_code_pkce", "refresh_token"], "authMethods": ["none"], "redirectUris": redirects,
        "allowedScopes": ["mcp", "offline_access"], "tenant": context["tenant"], "metadata": {}});
    let existing: Vec<&Value> = retained["oauthClients"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|value| value["id"] == request.client_id.as_str())
        .collect();
    ensure!(
        existing.is_empty() || existing == [&client],
        "the existing public-client registration differs; review it before replacing it"
    );
    let mut result = retained.clone();
    if existing.is_empty() {
        result["oauthClients"]
            .as_array_mut()
            .context("the catalog lists OAuth clients")?
            .push(client);
    }
    let tenant = context["tenant"].clone();
    let grants = result["authorization"]["grants"]
        .as_array_mut()
        .context("the catalog lists grants")?;
    for member in &members {
        let grant = json!({"role": "member", "subject": {"principal": member}, "tenant": tenant});
        if !grants.contains(&grant) {
            grants.push(grant);
        }
    }
    let selected = result["workContexts"]
        .as_array_mut()
        .and_then(|contexts| {
            contexts
                .iter_mut()
                .find(|value| value["id"] == request.context.as_str())
        })
        .context("the Work Context is there")?;
    let contributors: Vec<String> = selected["memberships"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|membership| membership["level"] == "contributor")
        .flat_map(|membership| {
            membership["principals"]
                .as_array()
                .cloned()
                .unwrap_or_default()
        })
        .filter_map(|principal| principal.as_str().map(str::to_owned))
        .collect();
    let missing: Vec<&String> = members
        .iter()
        .filter(|member| !contributors.contains(member))
        .collect();
    if !missing.is_empty() {
        selected["memberships"]
            .as_array_mut()
            .context("a Work Context lists memberships")?
            .push(json!({"level": "contributor", "principals": missing}));
    }
    Ok(result)
}

/// The Keycloak public client of a catalog's local client, and its mappers.
pub fn local_public_client(client: &Value, context: &Value) -> (Value, Vec<Value>) {
    let resource = client["allowedResources"][0]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let representation = json!({"clientId": client["id"], "protocol": "openid-connect", "enabled": true,
        "publicClient": true, "standardFlowEnabled": true, "serviceAccountsEnabled": false,
        "directAccessGrantsEnabled": false, "implicitFlowEnabled": false, "fullScopeAllowed": false,
        "redirectUris": client["redirectUris"], "attributes": {"pkce.code.challenge.method": "S256",
            "access.token.lifespan": "300", "access.token.header.type.rfc9068": "true", "use.refresh.tokens": "true"}});
    let mut mappers = vec![
        claim("client_id", client["id"].as_str().unwrap_or_default()),
        claim("https://veoveo.ai/claims/principal-kind", "user"),
        claim(
            "https://veoveo.ai/claims/tenant",
            context["tenant"].as_str().unwrap_or_default(),
        ),
        claim(
            "https://veoveo.ai/claims/work-context",
            context["id"].as_str().unwrap_or_default(),
        ),
        claim("https://veoveo.ai/claims/invocation-mode", "direct"),
    ];
    for name in ["principal", "initiator"] {
        mappers.push(json!({"name": name, "protocol": "openid-connect", "protocolMapper": "oidc-usermodel-property-mapper",
            "config": {"user.attribute": "id", "claim.name": format!("https://veoveo.ai/claims/{name}").replace('.', r"\."),
                "jsonType.label": "String", "access.token.claim": "true", "id.token.claim": "false", "userinfo.token.claim": "false"}}));
    }
    mappers.push(json!({"name": "audience", "protocol": "openid-connect", "protocolMapper": "oidc-audience-mapper",
        "config": {"included.custom.audience": resource, "access.token.claim": "true", "id.token.claim": "false"}}));
    (representation, mappers)
}

/// Configures only the public client: no user, role, secret or service client changes.
pub(crate) async fn configure_public_client(
    admin: &impl Admin,
    catalog: &Value,
    client_id: &str,
) -> Result<()> {
    let client = catalog["oauthClients"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|value| value["id"] == client_id)
        .context("the catalog has the client")?;
    let context = catalog["workContexts"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|value| value["id"] == client["defaultWorkContext"])
        .context("the catalog has its Work Context")?;
    let (representation, mappers) = local_public_client(client, context);
    let matches = admin
        .get(&format!("{REALM}/clients?clientId={client_id}"))
        .await?;
    let matches = matches.as_array().cloned().unwrap_or_default();
    ensure!(
        matches.len() <= 1,
        "Keycloak holds more than one client {client_id}"
    );
    if let Some(existing) = matches.first() {
        ensure!(
            existing["publicClient"] == true && existing["serviceAccountsEnabled"] != true,
            "refusing to replace a confidential or service-account client"
        );
    }
    let scopes = admin.get(&format!("{REALM}/client-scopes")).await?;
    let id = |name: &str| id_named(&scopes, "name", name);
    let key = put_client(admin, client_id, &representation).await?;
    set_scopes(
        admin,
        &key,
        &[id("basic")?],
        &[id("mcp")?, id("offline_access")?],
    )
    .await?;
    set_mappers(admin, &key, &mappers).await
}

/// Prepares a local public PKCE client: Keycloak's client and a private catalog
/// candidate, which is published separately through Hub's administrative route.
pub async fn prepare_public_client(
    repository: &Path,
    control_plane: &Path,
    output: &Path,
    request: &PublicClient,
) -> Result<()> {
    let retained: Value = serde_json::from_str(
        &std::fs::read_to_string(control_plane)
            .with_context(|| format!("cannot read {}", control_plane.display()))?,
    )?;
    let prepared = local_public_catalog(&retained, request)?;
    let admin = Keycloak::administrator(repository).await?;
    for member in &request.members {
        let id = uuid::Uuid::parse_str(member)?.to_string();
        let user = admin.get(&format!("{REALM}/users/{id}")).await?;
        ensure!(
            user["id"] == id.as_str()
                && user["enabled"] == true
                && user
                    .get("serviceAccountClientId")
                    .is_none_or(Value::is_null),
            "a selected member must be an enabled human user"
        );
    }
    configure_public_client(&admin, &prepared, &request.client_id).await?;
    write_private(
        output,
        &(serde_json::to_string_pretty(&prepared)? + "\n"),
        false,
    )?;
    println!(
        "Prepared the native public PKCE client and a private catalog candidate; nothing was published and no one signed in."
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use pretty_assertions::assert_eq;

    use super::*;

    /// Keycloak's administration API as recorded answers, with every call kept.
    #[derive(Default)]
    struct Recorded {
        calls: Mutex<Vec<(Method, String)>>,
        created: Mutex<bool>,
    }

    impl Admin for Recorded {
        async fn call(&self, method: Method, path: &str, _: Option<Value>) -> Result<Value> {
            self.calls
                .lock()
                .unwrap()
                .push((method.clone(), path.to_owned()));
            if path == format!("{REALM}/clients?clientId=opencode-local") {
                return Ok(if *self.created.lock().unwrap() {
                    json!([{"id": "fixture-client", "publicClient": true}])
                } else {
                    json!([])
                });
            }
            if path == format!("{REALM}/client-scopes") {
                return Ok(json!(
                    ["basic", "mcp", "offline_access"]
                        .map(|name| json!({"id": format!("{name}-scope"), "name": name}))
                ));
            }
            if path == format!("{REALM}/clients") && method == Method::POST {
                *self.created.lock().unwrap() = true;
            }
            Ok(if method == Method::GET {
                json!([])
            } else {
                Value::Null
            })
        }
    }

    const MEMBER: &str = "33333333-3333-4333-8333-333333333333";

    fn retained() -> Value {
        let mut retained = local_catalog(HUB_ORIGIN, true, true, false, None).unwrap();
        retained["workContexts"][0]["id"] = json!("chosen-work");
        retained["workContexts"][0]["tenant"] = json!("chosen-tenant");
        retained["tenants"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id": "chosen-tenant", "title": "Chosen"}));
        retained
    }

    fn request() -> PublicClient {
        PublicClient {
            client_id: "opencode-local".into(),
            members: vec![MEMBER.into()],
            context: "chosen-work".into(),
            resource: format!("{HUB_ORIGIN}/mcp/local"),
            redirects: vec!["http://127.0.0.1:3210/callback".into()],
        }
    }

    #[test]
    fn artifact_media_and_embedding_are_explicit_and_kept_on_a_rerun() {
        let baseline = local_catalog(HUB_ORIGIN, false, false, false, None).unwrap();
        assert_eq!(
            baseline["nativeUpstreams"]["bindings"],
            json!([binding("time")])
        );
        let selected = local_catalog(HUB_ORIGIN, true, false, false, None).unwrap();
        assert_eq!(
            selected["nativeUpstreams"]["bindings"],
            json!([binding("time"), binding("artifact")])
        );
        // A rerun without the flags keeps what was selected before.
        let rerun = local_catalog(HUB_ORIGIN, false, false, false, Some(&selected)).unwrap();
        assert_eq!(
            rerun["nativeUpstreams"]["bindings"],
            selected["nativeUpstreams"]["bindings"]
        );
        assert!(
            select_services(false, true, false, None).is_err(),
            "Media needs Artifact"
        );
        let mut foreign = selected.clone();
        foreign["nativeUpstreams"]["bindings"][1]["port"] = json!(9999);
        assert!(local_catalog(HUB_ORIGIN, false, false, false, Some(&foreign)).is_err());
        let embedding = local_catalog(HUB_ORIGIN, false, false, true, None).unwrap();
        assert_eq!(
            embedding["authorization"]["roles"], baseline["authorization"]["roles"],
            "Embedding adds no authority"
        );
    }

    #[test]
    fn media_adds_only_the_narrow_billed_role_and_client() {
        let media = local_catalog(HUB_ORIGIN, true, true, false, None).unwrap();
        let roles: Vec<&str> = media["authorization"]["roles"]
            .as_array()
            .unwrap()
            .iter()
            .map(|role| role["name"].as_str().unwrap())
            .collect();
        assert_eq!(roles, ["member", "admin", "billed"]);
        assert_eq!(
            media["authorization"]["roles"][2]["risks"],
            json!(["billed"])
        );
        assert_eq!(
            media["workContexts"][0]["memberships"][0]["oauthClients"],
            json!(["vv-hub-member", "vv-hub-admin", "vv-hub-billed"])
        );
    }

    #[test]
    fn only_the_explicit_member_and_client_are_added() {
        let retained = retained();
        let prepared = local_public_catalog(&retained, &request()).unwrap();
        assert_eq!(
            prepared["oauthClients"].as_array().unwrap().last().unwrap()["redirectUris"],
            json!(["http://127.0.0.1:3210/callback"])
        );
        assert_eq!(
            prepared["authorization"]["grants"]
                .as_array()
                .unwrap()
                .last()
                .unwrap(),
            &json!({"role": "member", "subject": {"principal": MEMBER}, "tenant": "chosen-tenant"})
        );
        assert_eq!(
            local_public_catalog(&prepared, &request()).unwrap(),
            prepared,
            "a rerun changes nothing"
        );
        let mut back = prepared.clone();
        back["oauthClients"].as_array_mut().unwrap().pop();
        back["authorization"]["grants"]
            .as_array_mut()
            .unwrap()
            .pop();
        back["workContexts"][0]["memberships"]
            .as_array_mut()
            .unwrap()
            .pop();
        assert_eq!(back, retained);
        for change in [
            PublicClient {
                context: "missing".into(),
                ..request()
            },
            PublicClient {
                members: vec![],
                ..request()
            },
            PublicClient {
                redirects: vec!["http://localhost:3210/callback".into()],
                ..request()
            },
        ] {
            assert!(local_public_catalog(&retained, &change).is_err());
        }
        let mut differing = prepared;
        differing["oauthClients"]
            .as_array_mut()
            .unwrap()
            .last_mut()
            .unwrap()["allowedScopes"]
            .as_array_mut()
            .unwrap()
            .push(json!("admin"));
        assert!(
            local_public_catalog(&differing, &request())
                .unwrap_err()
                .to_string()
                .contains("registration differs")
        );
    }

    #[test]
    fn a_public_client_has_no_secret_service_account_or_fixed_subject() {
        let prepared = local_public_catalog(&retained(), &request()).unwrap();
        let (client, mappers) = local_public_client(
            prepared["oauthClients"].as_array().unwrap().last().unwrap(),
            &prepared["workContexts"][0],
        );
        assert_eq!(client["publicClient"], true);
        for field in [
            "serviceAccountsEnabled",
            "directAccessGrantsEnabled",
            "implicitFlowEnabled",
            "fullScopeAllowed",
        ] {
            assert_eq!(client[field], false, "{field}");
        }
        assert!(client.get("secret").is_none());
        assert!(!mappers.iter().any(|mapper| mapper["name"] == "sub"));
        let principal = mappers
            .iter()
            .find(|mapper| mapper["name"] == "principal")
            .unwrap();
        assert_eq!(principal["config"]["user.attribute"], "id");
    }

    #[tokio::test]
    async fn configuring_the_public_client_touches_only_that_client() {
        let prepared = local_public_catalog(&retained(), &request()).unwrap();
        let admin = Recorded::default();
        configure_public_client(&admin, &prepared, "opencode-local")
            .await
            .unwrap();
        let calls = admin.calls.lock().unwrap();
        assert!(!calls.iter().any(|(_, path)| path.contains("/users")
            || path.contains("/roles")
            || path.contains("client-secret")));
        let writes: Vec<&(Method, String)> = calls
            .iter()
            .filter(|(method, _)| *method != Method::GET)
            .collect();
        assert!(writes.contains(&&(
            Method::PUT,
            format!("{REALM}/clients/fixture-client/optional-client-scopes/mcp-scope")
        )));
        assert!(writes.contains(&&(
            Method::PUT,
            format!("{REALM}/clients/fixture-client/optional-client-scopes/offline_access-scope")
        )));
        assert!(
            writes
                .iter()
                .all(|(_, path)| path == &format!("{REALM}/clients")
                    || path.starts_with(&format!("{REALM}/clients/fixture-client/")))
        );
    }

    #[test]
    fn a_fixture_environment_is_made_once_and_recovered() {
        let directory =
            std::env::temp_dir().join(format!("veoveo-installer-fixture-{}", std::process::id()));
        let path = directory.join("hub.env");
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            &path,
            "VEOVEO_ISSUER=https://vv-control--hub--http.openshell.localhost:17670\n",
        )
        .unwrap();
        let error = local_environment(&path, HUB_ORIGIN)
            .unwrap_err()
            .to_string();
        assert!(error.contains("never made again"), "{error}");
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "VEOVEO_ISSUER=https://vv-control--hub--http.openshell.localhost:17670\n"
        );
        std::fs::remove_dir_all(&directory).unwrap();
    }
}
