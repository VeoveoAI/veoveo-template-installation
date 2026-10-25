//! YAML read as Kubernetes and Helm read it: their parsers follow YAML 1.1, where a
//! number with a leading zero, such as a Secret volume's `defaultMode: 0440`, is octal.

use anyhow::{Result, anyhow};
use serde_json::Value;

fn options() -> serde_saphyr::Options {
    serde_saphyr::options! { legacy_octal_numbers: true }
}

/// One document.
pub fn value(text: &str) -> Result<Value> {
    serde_saphyr::from_str_with_options(text, options()).map_err(|error| anyhow!("{error}"))
}

/// Every document of a stream.
pub fn documents(text: &str) -> Result<Vec<Value>> {
    serde_saphyr::from_multiple_with_options(text, options()).map_err(|error| anyhow!("{error}"))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_leading_zero_is_octal_as_kubernetes_reads_it() {
        assert_eq!(
            value("defaultMode: 0440\nport: 8080\nname: '0440'\n").unwrap(),
            json!({"defaultMode": 288, "port": 8080, "name": "0440"})
        );
    }

    #[test]
    fn a_stream_holds_its_documents_in_order() {
        let documents = documents("a: 1\n---\nb: 2\n").unwrap();
        assert_eq!(documents, [json!({"a": 1}), json!({"b": 2})]);
    }
}
