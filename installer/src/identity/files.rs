//! Private files, environment files, generated secrets and Hub's Ed25519 keys. Nothing
//! here prints a secret: errors name files and variables, never values.

use std::{
    io::Write as _,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail, ensure};
use base64::Engine as _;
use ring::rand::SecureRandom as _;
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

/// Creates `directory` and its parents, owner-only.
pub fn private_directory(directory: &Path) -> Result<()> {
    std::fs::create_dir_all(directory)
        .with_context(|| format!("cannot create {}", directory.display()))?;
    set_mode(directory, 0o700)
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
        .with_context(|| format!("cannot set the mode of {}", path.display()))
}

#[cfg(not(unix))]
fn set_mode(_: &Path, _: u32) -> Result<()> {
    Ok(())
}

/// Writes an owner-only file. `exclusive` refuses one that exists, so a first write
/// never replaces retained material; otherwise a complete copy replaces it atomically.
pub fn write_private(path: &Path, text: &str, exclusive: bool) -> Result<()> {
    let directory = path.parent().context("a private file has a directory")?;
    private_directory(directory)?;
    if exclusive {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        let mut file = options
            .open(path)
            .with_context(|| format!("cannot create {}; it may exist already", path.display()))?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
    } else {
        let temporary = temporary_name(path);
        {
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
            let mut file = options
                .open(&temporary)
                .with_context(|| format!("cannot create {}", temporary.display()))?;
            file.write_all(text.as_bytes())?;
            file.sync_all()?;
        }
        std::fs::rename(&temporary, path)
            .with_context(|| format!("cannot replace {}", path.display()))?;
    }
    set_mode(path, 0o600)
}

fn temporary_name(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file");
    path.with_file_name(format!(".{name}.{}.next", std::process::id()))
}

/// `count` random bytes from the system's secure generator.
pub fn random_bytes(count: usize) -> Result<Vec<u8>> {
    let mut bytes = vec![0; count];
    ring::rand::SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| anyhow::anyhow!("the system's random generator failed"))?;
    Ok(bytes)
}

/// Standard base64 with padding, as Hub's `*_B64` variables hold their keys.
pub fn base64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// A URL-safe text of `count` random bytes, without padding.
pub fn token_urlsafe(count: usize) -> Result<String> {
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(random_bytes(count)?))
}

/// The PKCS#8 v1 prefix of an Ed25519 private key, RFC 8410 §7.
const ED25519_PKCS8_PREFIX: [u8; 16] = [
    0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20,
];

/// A new Ed25519 private key as PKCS#8 DER, as `openssl genpkey -algorithm Ed25519
/// -outform DER` writes it.
pub fn ed25519_key() -> Result<Vec<u8>> {
    let mut key = ED25519_PKCS8_PREFIX.to_vec();
    key.extend(random_bytes(32)?);
    Ok(key)
}

/// The public key of an Ed25519 PKCS#8 key.
pub fn ed25519_public(pkcs8: &[u8]) -> Result<[u8; 32]> {
    let seed = pkcs8
        .strip_prefix(&ED25519_PKCS8_PREFIX)
        .filter(|seed| seed.len() == 32)
        .context("Hub's signing key must be an Ed25519 PKCS#8 key")?;
    let pair = ring::signature::Ed25519KeyPair::from_seed_unchecked(seed)
        .map_err(|_| anyhow::anyhow!("Hub's signing key is not a valid Ed25519 key"))?;
    let public: [u8; 32] = ring::signature::KeyPair::public_key(&pair)
        .as_ref()
        .try_into()
        .context("an Ed25519 public key has 32 bytes")?;
    Ok(public)
}

/// The public JWK of Hub's access-token signing key, with its RFC 7638 thumbprint as
/// `kid`. Only the public key leaves.
pub fn authorization_jwk(key_base64: &str) -> Result<Value> {
    let key = base64::engine::general_purpose::STANDARD
        .decode(key_base64.trim())
        .context("the access-token signing key is not base64")?;
    let public = ed25519_public(&key)?;
    let x = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(public);
    // RFC 7638: the required members in lexicographic order, without whitespace.
    let canonical = format!(r#"{{"crv":"Ed25519","kty":"OKP","x":"{x}"}}"#);
    let kid = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(Sha256::digest(canonical.as_bytes()));
    Ok(json!({"crv": "Ed25519", "kty": "OKP", "x": x, "kid": kid, "alg": "EdDSA", "use": "sig"}))
}

/// An environment file of `NAME=value` lines, each value one shell word, as a shell
/// entrypoint sources it.
pub fn parse_environment(text: &str) -> Result<Vec<(String, String)>> {
    let mut values: Vec<(String, String)> = Vec::new();
    for (number, line) in text.lines().enumerate() {
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let (name, value) = line
            .split_once('=')
            .with_context(|| format!("line {} is not NAME=value", number + 1))?;
        ensure!(
            !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
            "line {} names no variable",
            number + 1
        );
        ensure!(
            !values.iter().any(|(existing, _)| existing == name),
            "{name} is set twice"
        );
        let value = shell_word(value).with_context(|| format!("{name} is not one shell word"))?;
        values.push((name.to_owned(), value));
    }
    Ok(values)
}

/// One POSIX shell word: unquoted, `'single'`, `"double"` and backslash escapes.
fn shell_word(text: &str) -> Result<String> {
    let mut word = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\'' => loop {
                match chars.next() {
                    Some('\'') => break,
                    Some(c) => word.push(c),
                    None => bail!("an open single quote"),
                }
            },
            '"' => loop {
                match chars.next() {
                    Some('"') => break,
                    Some('\\') => match chars.next() {
                        Some(c @ ('"' | '\\' | '$' | '`')) => word.push(c),
                        Some('\n') => {}
                        Some(c) => {
                            word.push('\\');
                            word.push(c);
                        }
                        None => bail!("an open double quote"),
                    },
                    Some('$' | '`') => bail!("an expansion"),
                    Some(c) => word.push(c),
                    None => bail!("an open double quote"),
                }
            },
            '\\' => word.push(chars.next().context("a trailing backslash")?),
            c if c.is_whitespace() => bail!("more than one word"),
            '$' | '`' | ';' | '&' | '|' | '<' | '>' | '(' | ')' => {
                bail!("a shell operator or expansion")
            }
            c => word.push(c),
        }
    }
    Ok(word)
}

/// `value` as one shell word, as Python's `shlex.quote` writes it.
pub fn shell_quote(value: &str) -> String {
    let safe = !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "@%+=:,./-_".contains(c));
    if safe {
        value.to_owned()
    } else {
        format!("'{}'", value.replace('\'', r#"'"'"'"#))
    }
}

pub fn render_environment(values: &[(String, String)]) -> String {
    values
        .iter()
        .map(|(name, value)| format!("{name}={}\n", shell_quote(value)))
        .collect()
}

/// A secret a trusted file holds: its whole content, which may not hold a line break.
pub fn read_secret(path: &Path) -> Result<String> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    ensure!(!text.is_empty(), "{} is empty", path.display());
    ensure!(
        !text.contains(['\n', '\r']),
        "{} holds a line break; write it without a trailing newline",
        path.display()
    );
    Ok(text)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn environment_values_round_trip_as_shell_words() {
        let values = vec![
            (
                "VEOVEO_ISSUER".to_owned(),
                "https://veoveo.example.com".to_owned(),
            ),
            (
                "VEOVEO_DATABASE_URL".to_owned(),
                "postgres://veoveo_owner@db:5432/veoveo?sslmode=verify-full&x=1".to_owned(),
            ),
            ("QUOTED".to_owned(), "it's".to_owned()),
            ("EMPTY".to_owned(), String::new()),
        ];
        let text = render_environment(&values);
        assert_eq!(
            text,
            "VEOVEO_ISSUER=https://veoveo.example.com\n\
             VEOVEO_DATABASE_URL='postgres://veoveo_owner@db:5432/veoveo?sslmode=verify-full&x=1'\n\
             QUOTED='it'\"'\"'s'\n\
             EMPTY=''\n"
        );
        assert_eq!(parse_environment(&text).unwrap(), values);
        assert_eq!(
            parse_environment("A=\"b c\"\n").unwrap(),
            [("A".to_owned(), "b c".to_owned())]
        );
    }

    #[test]
    fn a_malformed_environment_is_refused() {
        for text in [
            "A=b c\n",
            "A=$(id)\n",
            "A='open\n",
            "noequals\n",
            "A=1\nA=2\n",
            "=x\n",
        ] {
            assert!(parse_environment(text).is_err(), "{text:?}");
        }
    }

    #[test]
    fn an_ed25519_key_reads_back_as_its_public_jwk() {
        // RFC 8032 §7.1 test 1: the secret key and its public key.
        let seed = hex_bytes("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60");
        let public = hex_bytes("d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a");
        let mut key = ED25519_PKCS8_PREFIX.to_vec();
        key.extend(&seed);
        assert_eq!(ed25519_public(&key).unwrap().to_vec(), public);
        let jwk = authorization_jwk(&base64(&key)).unwrap();
        assert_eq!(jwk["x"], "11qYAYKxCrfVS_7TyWQHOg7hcvPapiMlrwIaaPcHURo");
        // RFC 8037 §A.3: the RFC 7638 thumbprint of that public key.
        assert_eq!(jwk["kid"], "kPrK_qmxVWaYVA9wwBF6Iuo3vVzz7TxHCTwXBygrS4k");
        assert_eq!(jwk["alg"], "EdDSA");
        assert!(ed25519_public(&key[1..]).is_err());
    }

    #[test]
    fn generated_keys_are_fresh_and_well_formed() {
        let a = ed25519_key().unwrap();
        let b = ed25519_key().unwrap();
        assert_eq!(a.len(), 48);
        assert_ne!(a, b);
        assert!(ed25519_public(&a).is_ok());
        assert_eq!(token_urlsafe(32).unwrap().len(), 43);
    }

    #[cfg(unix)]
    #[test]
    fn a_first_write_never_replaces_a_file() {
        use std::os::unix::fs::PermissionsExt as _;
        let directory =
            std::env::temp_dir().join(format!("veoveo-installer-files-{}", std::process::id()));
        let path = directory.join("hub.env");
        write_private(&path, "A=1\n", true).unwrap();
        assert!(write_private(&path, "A=2\n", true).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "A=1\n");
        write_private(&path, "A=3\n", false).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "A=3\n");
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::metadata(&directory).unwrap().permissions().mode() & 0o777,
            0o700
        );
        std::fs::remove_dir_all(&directory).unwrap();
    }

    fn hex_bytes(text: &str) -> Vec<u8> {
        (0..text.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
            .collect()
    }
}
