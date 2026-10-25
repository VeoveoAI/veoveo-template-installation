//! The difference between a declared value and the one that runs, as JSON trees. Objects
//! compare member by member; a list whose items all carry a distinct `id` or `name`
//! compares item by item under that key, any other list position by position. A member
//! one side lacks equals the other side's empty value (`null`, `""`, `0`, `false`, `[]`
//! or `{}`), since owners leave defaults out or write them back.

use std::fmt;

use serde_json::Value;

/// Where in a document a difference is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Path(Vec<Segment>);

#[derive(Debug, Clone, PartialEq, Eq)]
enum Segment {
    Member(String),
    Index(usize),
    /// A list item matched by its key member's value.
    Keyed {
        key: &'static str,
        value: String,
    },
}

impl Path {
    pub fn root() -> Self {
        Self(Vec::new())
    }

    pub fn member(&self, name: &str) -> Self {
        self.with(Segment::Member(name.to_owned()))
    }

    /// This path inside the member `name`.
    pub fn under(&self, name: &str) -> Self {
        let mut segments = vec![Segment::Member(name.to_owned())];
        segments.extend(self.0.iter().cloned());
        Self(segments)
    }

    fn with(&self, segment: Segment) -> Self {
        let mut segments = self.0.clone();
        segments.push(segment);
        Self(segments)
    }

    /// The last member name on the path, which decides whether its value is shown.
    pub fn last_member(&self) -> Option<&str> {
        self.0.iter().rev().find_map(|segment| match segment {
            Segment::Member(name) => Some(name.as_str()),
            _ => None,
        })
    }
}

impl fmt::Display for Path {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            return f.write_str("(the whole value)");
        }
        for (position, segment) in self.0.iter().enumerate() {
            match segment {
                Segment::Member(name) => {
                    let plain = !name.is_empty()
                        && name
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
                    match (position, plain) {
                        (0, true) => write!(f, "{name}")?,
                        (_, true) => write!(f, ".{name}")?,
                        (_, false) => write!(f, "[{name:?}]")?,
                    }
                }
                Segment::Index(index) => write!(f, "[{index}]")?,
                Segment::Keyed { key, value } => write!(f, "[{key}={value}]")?,
            }
        }
        Ok(())
    }
}

/// One difference. `declared` is what this repository says; `active` is what runs.
#[derive(Debug, Clone, PartialEq)]
pub struct Difference {
    pub path: Path,
    pub declared: Option<Value>,
    pub active: Option<Value>,
}

impl Difference {
    pub fn new(path: Path, declared: Option<Value>, active: Option<Value>) -> Self {
        Self {
            path,
            declared,
            active,
        }
    }
}

/// Every difference between `declared` and `active`: members by name, declared ones
/// first, and list items in declared order.
pub fn compare(declared: &Value, active: &Value) -> Vec<Difference> {
    let mut differences = Vec::new();
    walk(&Path::root(), declared, active, &mut differences);
    differences
}

fn walk(path: &Path, declared: &Value, active: &Value, out: &mut Vec<Difference>) {
    match (declared, active) {
        (Value::Object(declared), Value::Object(active)) => {
            for (name, value) in declared {
                let path = path.member(name);
                match active.get(name) {
                    Some(other) => walk(&path, value, other, out),
                    None if is_empty(value) => {}
                    None => out.push(Difference::new(path, Some(value.clone()), None)),
                }
            }
            for (name, value) in active {
                if !declared.contains_key(name) && !is_empty(value) {
                    out.push(Difference::new(
                        path.member(name),
                        None,
                        Some(value.clone()),
                    ));
                }
            }
        }
        (Value::Array(declared), Value::Array(active)) => match list_key(declared, active) {
            Some(key) => walk_keyed(path, key, declared, active, out),
            None => {
                for (index, value) in declared.iter().enumerate() {
                    let path = path.with(Segment::Index(index));
                    match active.get(index) {
                        Some(other) => walk(&path, value, other, out),
                        None => out.push(Difference::new(path, Some(value.clone()), None)),
                    }
                }
                for (index, value) in active.iter().enumerate().skip(declared.len()) {
                    out.push(Difference::new(
                        path.with(Segment::Index(index)),
                        None,
                        Some(value.clone()),
                    ));
                }
            }
        },
        (declared, active) if declared == active => {}
        (declared, active) if is_empty(declared) && is_empty(active) => {}
        (declared, active) => out.push(Difference::new(
            path.clone(),
            Some(declared.clone()),
            Some(active.clone()),
        )),
    }
}

fn walk_keyed(
    path: &Path,
    key: &'static str,
    declared: &[Value],
    active: &[Value],
    out: &mut Vec<Difference>,
) {
    let key_of = |item: &Value| item[key].as_str().unwrap_or_default().to_owned();
    for item in declared {
        let value = key_of(item);
        let path = path.with(Segment::Keyed {
            key,
            value: value.clone(),
        });
        match active.iter().find(|other| key_of(other) == value) {
            Some(other) => walk(&path, item, other, out),
            None => out.push(Difference::new(path, Some(item.clone()), None)),
        }
    }
    for item in active {
        let value = key_of(item);
        if !declared.iter().any(|other| key_of(other) == value) {
            out.push(Difference::new(
                path.with(Segment::Keyed { key, value }),
                None,
                Some(item.clone()),
            ));
        }
    }
}

/// The member that identifies every item of both lists, distinctly within each.
fn list_key(declared: &[Value], active: &[Value]) -> Option<&'static str> {
    if declared.is_empty() && active.is_empty() {
        return None;
    }
    ["id", "name"].into_iter().find(|key| {
        [declared, active].iter().all(|items| {
            let mut seen = std::collections::BTreeSet::new();
            items
                .iter()
                .all(|item| item[*key].as_str().is_some_and(|value| seen.insert(value)))
        })
    })
}

pub fn is_empty(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::Bool(value) => !value,
        Value::Number(number) => number.as_f64() == Some(0.0),
        Value::String(text) => text.is_empty(),
        Value::Array(items) => items.is_empty(),
        Value::Object(members) => members.is_empty(),
    }
}

/// Removes the member at `path` (member names separated by dots, `*` for every member
/// or list item) from `value`.
pub fn remove(value: &mut Value, path: &str) {
    let segments: Vec<&str> = path.split('.').collect();
    remove_at(value, &segments);
}

fn remove_at(value: &mut Value, segments: &[&str]) {
    let Some((first, rest)) = segments.split_first() else {
        return;
    };
    match value {
        Value::Object(members) if rest.is_empty() => {
            if *first == "*" {
                members.clear();
            } else {
                members.remove(*first);
            }
        }
        Value::Object(members) if *first == "*" => {
            members
                .values_mut()
                .for_each(|member| remove_at(member, rest));
        }
        Value::Object(members) => {
            if let Some(member) = members.get_mut(*first) {
                remove_at(member, rest);
            }
        }
        Value::Array(items) if *first == "*" => {
            items.iter_mut().for_each(|item| remove_at(item, rest));
        }
        _ => {}
    }
}

/// Helm's merge of values files: maps merge member by member, the later file wins, a
/// later `null` removes the member, and any other value replaces the earlier one whole.
pub fn merge_values(base: &mut Value, layer: Value) {
    match (base, layer) {
        (Value::Object(base), Value::Object(layer)) => {
            for (name, value) in layer {
                if value.is_null() {
                    base.remove(&name);
                } else if let Some(existing) = base.get_mut(&name) {
                    merge_values(existing, value);
                } else {
                    base.insert(name, value);
                }
            }
        }
        (base, layer) => *base = layer,
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::*;

    fn shown(differences: &[Difference]) -> Vec<String> {
        differences
            .iter()
            .map(|difference| {
                format!(
                    "{}: {} -> {}",
                    difference.path,
                    difference
                        .declared
                        .as_ref()
                        .map_or("absent".into(), Value::to_string),
                    difference
                        .active
                        .as_ref()
                        .map_or("absent".into(), Value::to_string)
                )
            })
            .collect()
    }

    #[test]
    fn equal_documents_have_no_difference() {
        let value = json!({"a": [1, 2], "b": {"c": "d"}});
        assert!(compare(&value, &value).is_empty());
    }

    #[test]
    fn members_compare_by_name_then_active_extras() {
        let declared = json!({"image": "a@sha256:1", "replicas": 1, "gone": "x"});
        let active = json!({"image": "a@sha256:2", "replicas": 1, "extra": true});
        assert_eq!(
            shown(&compare(&declared, &active)),
            [
                r#"gone: "x" -> absent"#,
                r#"image: "a@sha256:1" -> "a@sha256:2""#,
                "extra: absent -> true",
            ]
        );
    }

    #[test]
    fn a_missing_member_equals_an_empty_one() {
        let declared = json!({"description": "", "labels": {}, "items": []});
        let active = json!({"enforce": false, "count": 0, "note": null});
        assert!(compare(&declared, &active).is_empty());
    }

    #[test]
    fn lists_of_named_items_compare_by_name_whatever_their_order() {
        let declared = json!({"containers": [
            {"name": "agent", "image": "a"},
            {"name": "sidecar", "image": "s"},
        ]});
        let active = json!({"containers": [
            {"name": "sidecar", "image": "s"},
            {"name": "agent", "image": "b"},
            {"name": "debug", "image": "d"},
        ]});
        assert_eq!(
            shown(&compare(&declared, &active)),
            [
                r#"containers[name=agent].image: "a" -> "b""#,
                r#"containers[name=debug]: absent -> {"image":"d","name":"debug"}"#,
            ]
        );
    }

    #[test]
    fn identified_items_prefer_id_and_other_lists_compare_by_position() {
        let declared = json!({"clients": [{"id": "cli", "name": "x"}], "args": ["a", "b"]});
        let active = json!({"clients": [{"id": "cli", "name": "y"}], "args": ["a"]});
        assert_eq!(
            shown(&compare(&declared, &active)),
            [
                r#"args[1]: "b" -> absent"#,
                r#"clients[id=cli].name: "x" -> "y""#
            ]
        );
    }

    #[test]
    fn a_list_with_a_repeated_key_compares_by_position() {
        let declared = json!([{"name": "a", "v": 1}, {"name": "a", "v": 2}]);
        let active = json!([{"name": "a", "v": 1}, {"name": "a", "v": 3}]);
        assert_eq!(shown(&compare(&declared, &active)), ["[1].v: 2 -> 3"]);
    }

    #[test]
    fn paths_quote_members_that_are_not_plain_names() {
        let declared = json!({"metadata": {"annotations": {"veoveo.ai/release-note": "a"}}});
        let active = json!({"metadata": {"annotations": {"veoveo.ai/release-note": "b"}}});
        assert_eq!(
            compare(&declared, &active)[0].path.to_string(),
            r#"metadata.annotations["veoveo.ai/release-note"]"#
        );
    }

    #[test]
    fn remove_takes_members_and_wildcards() {
        let mut value = json!({
            "metadata": {"uid": "u", "name": "n"},
            "items": [{"memberships": [1], "id": "a"}, {"id": "b"}],
        });
        remove(&mut value, "metadata.uid");
        remove(&mut value, "items.*.memberships");
        remove(&mut value, "absent.path");
        assert_eq!(
            value,
            json!({"metadata": {"name": "n"}, "items": [{"id": "a"}, {"id": "b"}]})
        );
    }

    #[test]
    fn values_merge_as_helm_merges_them() {
        let mut values = json!({"a": {"b": 1, "c": [1, 2]}, "d": "keep", "e": 1});
        merge_values(&mut values, json!({"a": {"c": [3], "f": true}, "e": null}));
        assert_eq!(
            values,
            json!({"a": {"b": 1, "c": [3], "f": true}, "d": "keep"})
        );
    }
}
