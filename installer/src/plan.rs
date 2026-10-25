//! A plan: for each declared step, in order, what differs between this repository and
//! what runs. Rendering shows only what differs or could not be read.

use std::fmt::Write as _;

use serde_json::Value;

use crate::difference::Difference;

#[derive(Debug)]
pub struct Plan {
    pub steps: Vec<StepPlan>,
}

#[derive(Debug)]
pub struct StepPlan {
    pub name: String,
    pub owner: &'static str,
    pub items: Vec<Item>,
}

/// One declared thing and how it stands.
#[derive(Debug)]
pub struct Item {
    /// What it is, such as `StatefulSet veoveo-system/registry` or `sandbox vv-control/hub`.
    pub subject: String,
    pub state: State,
}

#[derive(Debug)]
pub enum State {
    /// What runs is what this repository declares.
    Same,
    /// It does not exist; apply would create it.
    Absent { note: Option<String> },
    /// It differs; apply would change these.
    Differs {
        differences: Vec<Difference>,
        note: Option<String>,
    },
    /// The owner refuses the declared state as it stands.
    Refused(String),
    /// It exists but this repository does not declare it.
    Undeclared(String),
    /// What runs could not be read, so it was not compared.
    Unread(String),
    /// Deliberately not compared, and why.
    NotCompared(String),
}

impl Item {
    pub fn new(subject: impl Into<String>, state: State) -> Self {
        Self {
            subject: subject.into(),
            state,
        }
    }

    /// `Same` when there is no difference.
    pub fn compared(
        subject: impl Into<String>,
        differences: Vec<Difference>,
        note: Option<String>,
    ) -> Self {
        let state = if differences.is_empty() {
            State::Same
        } else {
            State::Differs { differences, note }
        };
        Self::new(subject, state)
    }

    pub fn unread(subject: impl Into<String>, reason: impl std::fmt::Display) -> Self {
        Self::new(subject, State::Unread(reason.to_string()))
    }

    fn reported(&self) -> bool {
        !matches!(self.state, State::Same)
    }
}

impl StepPlan {
    pub fn new(name: &str, owner: &'static str, items: Vec<Item>) -> Self {
        Self {
            name: name.to_owned(),
            owner,
            items,
        }
    }

    pub fn matches(&self) -> bool {
        self.items
            .iter()
            .all(|item| matches!(item.state, State::Same))
    }
}

impl Plan {
    /// Whether every step was read and matches.
    pub fn matches(&self) -> bool {
        self.steps.iter().all(StepPlan::matches)
    }

    /// The ordered list of what differs and what could not be read.
    pub fn render(&self) -> String {
        let mut out = String::new();
        let total = self.steps.len();
        for (position, step) in self.steps.iter().enumerate() {
            if step.matches() {
                continue;
            }
            let _ = writeln!(out, "{}. {} ({})", position + 1, step.name, step.owner);
            for item in step.items.iter().filter(|item| item.reported()) {
                render_item(&mut out, item);
            }
            out.push('\n');
        }
        let matching = self.steps.iter().filter(|step| step.matches()).count();
        let _ = writeln!(out, "{matching} of {total} steps match this repository.");
        out
    }
}

fn render_item(out: &mut String, item: &Item) {
    let subject = &item.subject;
    match &item.state {
        State::Same => {}
        State::Absent { note } => {
            let _ = writeln!(out, "   + {subject}: absent; apply creates it");
            if let Some(note) = note {
                let _ = writeln!(out, "       {note}");
            }
        }
        State::Differs { differences, note } => {
            let _ = writeln!(out, "   ~ {subject}");
            if let Some(note) = note {
                let _ = writeln!(out, "       {note}");
            }
            for difference in differences {
                render_difference(out, difference);
            }
        }
        State::Refused(reason) => {
            let _ = writeln!(out, "   x {subject}: refused: {reason}");
        }
        State::Undeclared(note) => {
            let _ = writeln!(out, "   ? {subject}: runs but is not declared; {note}");
        }
        State::Unread(reason) => {
            let _ = writeln!(out, "   ! {subject}: not read: {reason}");
        }
        State::NotCompared(reason) => {
            let _ = writeln!(out, "   - {subject}: not compared: {reason}");
        }
    }
}

fn render_difference(out: &mut String, difference: &Difference) {
    let path = &difference.path;
    let secret = path.last_member().is_some_and(is_secret_name);
    let show = |value: &Value| shown(value, secret);
    match (&difference.declared, &difference.active) {
        (Some(Value::String(declared)), Some(Value::String(active)))
            if declared.contains('\n') || active.contains('\n') =>
        {
            let _ = writeln!(out, "       {path}: text differs");
            if secret {
                let _ = writeln!(out, "         <redacted>");
            } else {
                render_text(out, declared, active);
            }
        }
        (Some(declared), Some(active)) => {
            let _ = writeln!(
                out,
                "       {path}: declared {}, active {}",
                show(declared),
                show(active)
            );
        }
        (Some(declared), None) => {
            let _ = writeln!(out, "       {path}: declared {}, absent", show(declared));
        }
        (None, Some(active)) => {
            let _ = writeln!(out, "       {path}: not declared, active {}", show(active));
        }
        (None, None) => {}
    }
}

/// The lines that differ, as declared (`+`) and active (`-`) lines, unified.
fn render_text(out: &mut String, declared: &str, active: &str) {
    const MAX_LINES: usize = 40;
    let diff = similar::TextDiff::from_lines(active, declared);
    let mut written = 0;
    for group in diff.grouped_ops(0) {
        for op in group {
            for change in diff.iter_changes(&op) {
                let sign = match change.tag() {
                    similar::ChangeTag::Delete => '-',
                    similar::ChangeTag::Insert => '+',
                    similar::ChangeTag::Equal => continue,
                };
                if written == MAX_LINES {
                    let _ = writeln!(out, "         (more lines differ)");
                    return;
                }
                let line = redact_text(change.value().trim_end_matches('\n'));
                let _ = writeln!(out, "         {sign} {}", truncate(&line, 160));
                written += 1;
            }
        }
    }
    let _ = writeln!(
        out,
        "         (+ declared, - active; {} and {} bytes)",
        declared.len(),
        active.len()
    );
}

/// A value as shown in a plan: compact JSON, truncated, with secrets redacted.
fn shown(value: &Value, secret: bool) -> String {
    if secret && !is_reference(value) {
        return "<redacted>".to_owned();
    }
    let mut value = value.clone();
    redact(&mut value);
    truncate(&value.to_string(), 200)
}

fn truncate(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_owned();
    }
    let kept: String = text.chars().take(limit).collect();
    format!("{kept}… ({} bytes)", text.len())
}

/// Whether a member's name says it holds a secret.
pub fn is_secret_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    [
        "password",
        "passwd",
        "secret",
        "token",
        "bearer",
        "privatekey",
        "private_key",
        "apikey",
        "api_key",
        "credential",
    ]
    .iter()
    .any(|word| name.contains(word))
}

/// A value that names a secret rather than holding one: an environment variable's name,
/// a path or a URL without credentials.
fn is_reference(value: &Value) -> bool {
    match value {
        Value::String(text) => {
            let variable = !text.is_empty()
                && text
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
            let path = text.starts_with('/');
            let url = url::Url::parse(text).is_ok_and(|url| {
                url.has_host() && url.username().is_empty() && url.password().is_none()
            });
            variable || path || url
        }
        Value::Bool(_) | Value::Number(_) | Value::Null => true,
        _ => false,
    }
}

/// Redacts members whose names say they hold secrets, and text that looks like a token
/// or a private key, anywhere in `value`.
pub fn redact(value: &mut Value) {
    match value {
        Value::Object(members) => {
            for (name, member) in members.iter_mut() {
                if is_secret_name(name)
                    && !is_reference(member)
                    && !member.is_object()
                    && !member.is_array()
                {
                    *member = Value::String("<redacted>".to_owned());
                } else {
                    redact(member);
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(redact),
        Value::String(text) if looks_secret(text) => {
            *text = "<redacted>".to_owned();
        }
        _ => {}
    }
}

fn redact_text(line: &str) -> String {
    if looks_secret(line) {
        "<redacted line>".to_owned()
    } else {
        line.to_owned()
    }
}

/// A JSON Web Token or PEM private key.
fn looks_secret(text: &str) -> bool {
    text.contains("PRIVATE KEY")
        || text
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.'))
            .any(|word| {
                word.starts_with("eyJ") && word.matches('.').count() == 2 && word.len() > 40
            })
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::*;
    use crate::difference::compare;

    fn plan() -> Plan {
        Plan {
            steps: vec![
                StepPlan::new(
                    "namespaces",
                    "Kubernetes",
                    vec![Item::compared("Namespace veoveo-system", vec![], None)],
                ),
                StepPlan::new(
                    "registry",
                    "Kubernetes",
                    vec![
                        Item::compared(
                            "StatefulSet veoveo-system/registry",
                            compare(
                                &json!({"image": "registry@sha256:1"}),
                                &json!({"image": "registry@sha256:2"}),
                            ),
                            None,
                        ),
                        Item::new(
                            "Job veoveo-system/seed",
                            State::NotCompared("a Job runs once".into()),
                        ),
                    ],
                ),
                StepPlan::new(
                    "hub",
                    "Hub",
                    vec![Item::unread(
                        "control plane",
                        "no sign-in to https://hub.example for profile local",
                    )],
                ),
            ],
        }
    }

    #[test]
    fn a_plan_lists_only_what_differs_in_step_order() {
        let plan = plan();
        assert!(!plan.matches());
        assert_eq!(
            plan.render(),
            "2. registry (Kubernetes)\n   \
             ~ StatefulSet veoveo-system/registry\n       \
             image: declared \"registry@sha256:1\", active \"registry@sha256:2\"\n   \
             - Job veoveo-system/seed: not compared: a Job runs once\n\n\
             3. hub (Hub)\n   \
             ! control plane: not read: no sign-in to https://hub.example for profile local\n\n\
             1 of 3 steps match this repository.\n"
        );
    }

    #[test]
    fn text_shows_the_lines_that_differ() {
        let differences = compare(
            &json!({"handbook": "# Handbook\nUse Lore.\nUse the Hub CLI.\n"}),
            &json!({"handbook": "# Handbook\nUse Lore.\n"}),
        );
        let mut out = String::new();
        render_difference(&mut out, &differences[0]);
        assert_eq!(
            out,
            "       handbook: text differs\n         \
             + Use the Hub CLI.\n         \
             (+ declared, - active; 38 and 21 bytes)\n"
        );
    }

    #[test]
    fn secrets_are_redacted_and_references_are_shown() {
        let differences = compare(
            &json!({"env": {"API_TOKEN": "s3cret", "credentialKey": "COMPUTER_HUB_REFERENCE"}}),
            &json!({"env": {"API_TOKEN": "0ther", "credentialKey": "LORE_ACCESS_TOKEN"}}),
        );
        let mut out = String::new();
        differences
            .iter()
            .for_each(|difference| render_difference(&mut out, difference));
        assert_eq!(
            out,
            "       env.API_TOKEN: declared <redacted>, active <redacted>\n       \
             env.credentialKey: declared \"COMPUTER_HUB_REFERENCE\", active \"LORE_ACCESS_TOKEN\"\n"
        );
        let mut value = json!({"nested": {"client_secret": "x"}, "jwt": "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.c2lnbmF0dXJl"});
        redact(&mut value);
        assert_eq!(
            value,
            json!({"nested": {"client_secret": "<redacted>"}, "jwt": "<redacted>"})
        );
    }
}
