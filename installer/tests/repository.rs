//! The repository this installer lives in: its declaration parses, names only files that
//! exist, and its lock decodes with core's installation contract.

use std::path::Path;

use veoveo_installer::{declaration::Declaration, inputs::read_lock};

fn repository() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the installer is a directory of its repository")
}

#[test]
fn the_declaration_names_files_that_exist() {
    let declaration = Declaration::read(repository()).unwrap();
    assert!(!declaration.steps.is_empty());
    let missing: Vec<_> = declaration
        .files()
        .into_iter()
        .filter(|file| !repository().join(file).exists())
        .collect();
    assert!(
        missing.is_empty(),
        "installation.toml names missing files: {missing:?}"
    );
}

#[test]
fn the_lock_decodes() {
    read_lock(repository()).unwrap();
}

#[test]
fn the_hub_seed_merges_and_names_hubs_origin() {
    use veoveo_installer::declaration::StepKind;
    let declaration = Declaration::read(repository()).unwrap();
    let hub = declaration
        .steps
        .iter()
        .find_map(|step| match &step.kind {
            StepKind::Hub(hub) => Some(hub),
            _ => None,
        })
        .expect("a Hub step");
    let mut seed = serde_json::json!({});
    for file in &hub.seed {
        let text = std::fs::read_to_string(repository().join(file)).unwrap();
        veoveo_installer::hub::merge_seed(&mut seed, serde_json::from_str(&text).unwrap());
    }
    let origin = veoveo_installer::hub::origin(&seed, &hub.profile).unwrap();
    assert_eq!(origin.scheme(), "https");
    // Entries of one id merge into one.
    let entra = seed["oidcClients"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|client| client["id"] == "entra")
        .count();
    assert_eq!(entra, 1);
}
