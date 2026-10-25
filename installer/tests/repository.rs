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

/// The repository's inputs with a lock naming every image the declaration uses, and
/// private deployment IDs for each sandbox that takes one, as recorded states.
fn inputs() -> veoveo_installer::inputs::Inputs {
    use veoveo_installer::{declaration::StepKind, private::Private};
    let declaration = Declaration::read(repository()).unwrap();
    let digest = "sha256:1111111111111111111111111111111111111111111111111111111111111111";
    let mut images = serde_json::Map::new();
    let mut private = String::new();
    for step in &declaration.steps {
        match &step.kind {
            StepKind::Sandboxes(sandboxes) => {
                for sandbox in sandboxes {
                    images.insert(
                        sandbox.image.clone(),
                        format!(
                            "registry.example.com:5000/veoveo/{}@{digest}",
                            sandbox.image
                        )
                        .into(),
                    );
                    if !sandbox.private_environment.is_empty() {
                        private.push_str(&format!("[sandbox.\"{}\".environment]\n", sandbox.key()));
                        for key in &sandbox.private_environment {
                            private.push_str(&format!(
                                "{key} = \"0199a0a0-0000-7000-8000-000000000001\"\n"
                            ));
                        }
                    }
                }
            }
            StepKind::Templates(templates) => {
                for image in templates
                    .iter()
                    .filter_map(|template| template.image.clone())
                {
                    images.insert(
                        image.clone(),
                        format!("registry.example.com:5000/veoveo/{image}@{digest}").into(),
                    );
                }
            }
            _ => {}
        }
    }
    for image in [
        "lore",
        "hub-sdk-cli",
        "openshell-gateway",
        "openshell-supervisor",
        "openshell-sandbox",
    ] {
        images.insert(
            image.to_owned(),
            format!("registry.example.com:5000/veoveo/{image}@{digest}").into(),
        );
    }
    let lock = serde_json::from_value(serde_json::json!({
        "format": "https://veoveo.ai/formats/component-lock",
        "components": [{
            "name": "veoveo-hub",
            "repository": "https://github.com/VeoveoAI/veoveo-hub",
            "revision": "0123456789abcdef0123456789abcdef01234567"
        }],
        "images": images,
    }))
    .unwrap();
    veoveo_installer::inputs::Inputs {
        repository: repository().to_path_buf(),
        declaration,
        lock,
        private: Private::parse(&private).unwrap(),
    }
}

#[test]
fn every_declared_sandbox_and_template_reads_back_as_declared_once_created() {
    use veoveo_installer::{difference::compare, openshell};
    let inputs = inputs();
    let mut sandboxes = 0;
    for sandbox in inputs.declaration.sandboxes() {
        let request = openshell::sandbox_request(&inputs, sandbox)
            .unwrap_or_else(|error| panic!("sandbox {}: {error:#}", sandbox.key()));
        let declared = openshell::requested_sandbox(&request).unwrap();
        // The gateway canonicalizes the policy it admits again; that changes nothing.
        let mut admitted = request.spec.clone().unwrap();
        admitted.policy = Some(
            openshell_policy::validate_and_canonicalize_sandbox_policy(admitted.policy.unwrap())
                .unwrap(),
        );
        let live = openshell::active_sandbox(&openshell_sdk::raw::proto::Sandbox {
            spec: Some(admitted),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(
            compare(&declared, &live),
            vec![],
            "sandbox {}",
            sandbox.key()
        );
        sandboxes += 1;
    }
    assert!(sandboxes > 0);
    for step in &inputs.declaration.steps {
        if let veoveo_installer::declaration::StepKind::Templates(templates) = &step.kind {
            for template in templates {
                let created =
                    openshell::template_proto(&inputs, template, Some("What is new".into()))
                        .unwrap();
                let without_note = openshell::template_proto(&inputs, template, None).unwrap();
                assert_eq!(
                    compare(
                        &openshell::active_template(&created),
                        &openshell::active_template(&without_note)
                    ),
                    vec![],
                    "template {}",
                    template.name
                );
            }
        }
    }
}

#[test]
fn the_seed_names_each_claim_file_by_its_content() {
    use veoveo_installer::{declaration::StepKind, hub};
    let inputs = inputs();
    let declared = inputs
        .declaration
        .steps
        .iter()
        .find_map(|step| match &step.kind {
            StepKind::Hub(hub) => Some(hub),
            _ => None,
        })
        .unwrap();
    let seed = hub::declared_seed(&inputs, declared).unwrap();
    let claim = inputs.declaration.claim(&declared.claim).unwrap();
    let files = hub::claim_files(&inputs, claim).unwrap();
    let named = hub::named_files(&seed, &claim.directory);
    for (_, file, _) in &files {
        assert!(named.contains(file), "the seed names no {file}");
    }
    assert!(!seed.to_string().contains("@claim:"));
}

/// The defaults of the charts whose values name images by digest, as recorded from
/// `helm show values` of the OpenShell chart at 491b3503d39f3ba83a25a309f52e52e20e304b03.
fn chart_defaults(release: &str) -> serde_json::Value {
    match release {
        "openshell" => serde_json::json!({
            "global": {"image": {"registry": "ghcr.io/nvidia"}},
            "gateway": {"image": {"registry": "", "repository": "openshell/gateway"}},
            "sandboxRuntime": {"image": {"registry": "", "repository": "openshell/sandbox"}},
            "supervisor": {"image": {"registry": "", "repository": "openshell/supervisor"}},
        }),
        _ => serde_json::json!({}),
    }
}

#[test]
fn the_helm_values_name_the_lock_repository_of_each_digest() {
    use veoveo_installer::{declaration::StepKind, helm, inputs::read_lock};
    let lock = read_lock(repository()).unwrap();
    let declaration = Declaration::read(repository()).unwrap();
    for step in &declaration.steps {
        let StepKind::Helm(release) = &step.kind else {
            continue;
        };
        let mut values = serde_json::json!({});
        for file in &release.values {
            let text = std::fs::read_to_string(repository().join(file)).unwrap();
            veoveo_installer::difference::merge_values(
                &mut values,
                veoveo_installer::yaml::value(&text).unwrap(),
            );
        }
        let digests = values.to_string().matches("@digest:").count();
        // A lock that names no image yet, as the template's, cannot be checked.
        if digests == 0 || lock.images().is_empty() {
            continue;
        }
        helm::digest_checks(
            &helm::with_defaults(chart_defaults(&release.release), &values),
            &lock,
        )
        .unwrap_or_else(|error| panic!("{}: {error:#}", release.release));
    }
}

#[test]
fn a_single_step_connects_every_owner_its_apply_needs() {
    use veoveo_installer::{apply, run};
    let inputs = inputs();
    // The OpenShell step that holds Hub's sandbox: replacing Hub is followed by waiting
    // for the workloads that become ready once Hub serves, in Kubernetes.
    let steps = run::selected(&inputs, &["hub".to_owned()]).unwrap();
    let selected: Vec<_> = steps.iter().map(|(_, step)| *step).collect();
    assert!(apply::awaits_hub_dependents(&inputs, selected[0]));
    assert_eq!(
        run::Needs::of_steps(&inputs, &selected),
        run::Needs {
            cluster: true,
            gateway: true
        }
    );
    // Every step reaches what its plan and its apply use.
    for step in &inputs.declaration.steps {
        if apply::awaits_hub_dependents(&inputs, step) {
            assert!(run::Needs::of(&inputs, step).cluster, "{}", step.name);
        }
    }
    let profiles = run::selected(&inputs, &["profiles".to_owned()]).unwrap();
    assert_eq!(
        run::Needs::of(&inputs, profiles[0].1),
        run::Needs {
            cluster: false,
            gateway: true
        }
    );
}
