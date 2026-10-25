//! The installer's inputs: this repository (its declaration, the component lock and the
//! files steps name) and the operator's private directory. Every image digest comes from
//! the lock: a file names an image as `@image:<name>@`, and filling it is the only way a
//! digest enters it.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use veoveo_installation_contract::{ComponentLock, ImageName};

use crate::{declaration::Declaration, private::Private};

pub const LOCK_FILE: &str = "component-lock.json";

/// A marker the template leaves for the operator to render; a file holding one is not
/// ready to compare.
const RENDER_MARKER: &str = "REPLACE_WITH_";

pub struct Inputs {
    pub repository: PathBuf,
    pub declaration: Declaration,
    pub lock: ComponentLock,
    pub private: Private,
}

impl Inputs {
    pub fn read(repository: &Path, private: &Path) -> Result<Self> {
        let declaration = Declaration::read(repository)?;
        let lock = read_lock(repository)?;
        let private = Private::read(private)?;
        Ok(Self {
            repository: repository.to_path_buf(),
            declaration,
            lock,
            private,
        })
    }

    /// A repository file, by its path relative to the repository.
    pub fn path(&self, relative: &Path) -> PathBuf {
        self.repository.join(relative)
    }

    pub fn text(&self, relative: &Path) -> Result<String> {
        let path = self.path(relative);
        std::fs::read_to_string(&path).with_context(|| format!("cannot read {}", path.display()))
    }

    /// A repository file with each image filled from the lock. A file that still holds
    /// a placeholder or a render marker afterwards is an error.
    pub fn filled(&self, relative: &Path) -> Result<String> {
        let text = self.text(relative)?;
        fill(&text, &self.lock).with_context(|| format!("{} cannot be filled", relative.display()))
    }

    /// The pinned reference of the image the lock names `name`.
    pub fn image(&self, name: &str) -> Result<String> {
        image(&self.lock, name)
    }
}

pub fn read_lock(repository: &Path) -> Result<ComponentLock> {
    let path = repository.join(LOCK_FILE);
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("cannot read {}", path.display()))?;
    serde_json::from_str(&text)
        .with_context(|| format!("{} is not a component lock", path.display()))
}

pub fn image(lock: &ComponentLock, name: &str) -> Result<String> {
    let image_name: ImageName = name
        .parse()
        .map_err(|error| anyhow!("{name:?} is not an image name: {error}"))?;
    lock.image(&image_name)
        .map(|image| image.to_string())
        .ok_or_else(|| anyhow!("the component lock names no image {name}"))
}

fn replace(text: &str, open: &str, value: impl Fn(&str) -> Result<String>) -> Result<String> {
    let mut filled = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(open) {
        filled.push_str(&rest[..start]);
        let after = &rest[start + open.len()..];
        let end = after
            .find('@')
            .ok_or_else(|| anyhow!("an {open} placeholder is not closed"))?;
        filled.push_str(&value(&after[..end])?);
        rest = &after[end + 1..];
    }
    filled.push_str(rest);
    Ok(filled)
}

/// Replaces every `@image:<name>@` with the lock's reference for `<name>`, and every
/// `@digest:<name>@` with its digest alone, for a file such as a chart's values that
/// names an image's repository and digest apart.
pub fn fill(text: &str, lock: &ComponentLock) -> Result<String> {
    let filled = replace(text, "@image:", |name| image(lock, name))?;
    let filled = replace(&filled, "@digest:", |name| {
        let reference = image(lock, name)?;
        Ok(reference
            .split_once('@')
            .map(|(_, digest)| digest.to_owned())
            .unwrap_or(reference))
    })?;
    if let Some(line) = filled.lines().find(|line| line.contains(RENDER_MARKER)) {
        bail!("it holds the unrendered marker in {:?}", line.trim());
    }
    Ok(filled)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) const DIGEST: &str =
        "sha256:1111111111111111111111111111111111111111111111111111111111111111";

    pub(crate) fn lock() -> ComponentLock {
        serde_json::from_value(serde_json::json!({
            "format": "https://veoveo.ai/formats/component-lock",
            "components": [{
                "name": "veoveo-hub",
                "repository": "https://github.com/VeoveoAI/veoveo-hub",
                "revision": "0123456789abcdef0123456789abcdef01234567"
            }],
            "images": {
                "lore": format!("registry.example.com:5000/veoveo/lore@{DIGEST}"),
                "hub-sdk-cli": format!("registry.example.com:5000/veoveo/hub-sdk-cli@{DIGEST}")
            }
        }))
        .unwrap()
    }

    #[test]
    fn placeholders_take_the_locked_digest() {
        let filled = fill(
            r#"{"lore": "@image:lore@", "cli": "@image:hub-sdk-cli@"}"#,
            &lock(),
        )
        .unwrap();
        assert_eq!(
            filled,
            format!(
                r#"{{"lore": "registry.example.com:5000/veoveo/lore@{DIGEST}", "cli": "registry.example.com:5000/veoveo/hub-sdk-cli@{DIGEST}"}}"#
            )
        );
    }

    #[test]
    fn a_digest_placeholder_takes_the_locked_digest_alone() {
        let filled = fill(
            "gateway:\n  image:\n    digest: \"@digest:lore@\"\n",
            &lock(),
        )
        .unwrap();
        assert_eq!(
            filled,
            format!("gateway:\n  image:\n    digest: \"{DIGEST}\"\n")
        );
    }

    #[test]
    fn an_image_the_lock_does_not_name_is_an_error() {
        let error = fill("image: @image:postgres@", &lock()).unwrap_err();
        assert_eq!(
            error.to_string(),
            "the component lock names no image postgres"
        );
    }

    #[test]
    fn an_unrendered_marker_is_an_error() {
        let error = fill(
            "image: REPLACE_WITH_REVIEWED_COPY_IMAGE_AT_SHA256\n",
            &lock(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("unrendered marker"), "{error}");
    }
}
