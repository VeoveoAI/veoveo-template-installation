//! Planning every declared step in order, each through its owner's interface.

use anyhow::Result;

use crate::{
    declaration::{Step, StepKind},
    helm, hub,
    inputs::Inputs,
    kubernetes::{self, Cluster},
    openshell::{self, Gateway},
    plan::{Item, Plan, StepPlan},
};

/// Plans the steps named in `only`, or every step. It reads and changes nothing.
pub async fn plan(inputs: &Inputs, only: &[String]) -> Result<Plan> {
    let steps: Vec<&Step> = inputs
        .declaration
        .steps
        .iter()
        .filter(|step| only.is_empty() || only.contains(&step.name))
        .collect();
    if let Some(unknown) = only
        .iter()
        .find(|name| inputs.declaration.step(name).is_none())
    {
        anyhow::bail!("installation.toml declares no step {unknown}");
    }
    let needs = |test: fn(&StepKind) -> bool| steps.iter().any(|step| test(&step.kind));
    let mut cluster = if needs(|kind| matches!(kind, StepKind::Kubernetes(_))) {
        Some(
            kubernetes::connect(inputs.private.kubernetes.context.as_deref())
                .await
                .map(Cluster::new)
                .map_err(|error| format!("{error:#}")),
        )
    } else {
        None
    };
    let gateway = if needs(|kind| {
        matches!(
            kind,
            StepKind::Workspaces(_)
                | StepKind::Profiles(_)
                | StepKind::Templates(_)
                | StepKind::Sandboxes(_)
                | StepKind::Hub(_)
        )
    }) {
        Some(match &inputs.private.openshell {
            Some(access) => openshell::connect(access)
                .await
                .map_err(|error| format!("{error:#}")),
            None => Err(
                "private.toml names no OpenShell gateway sign-in ([openshell] gateway)".to_owned(),
            ),
        })
    } else {
        None
    };
    let mut planned = Vec::with_capacity(steps.len());
    for step in steps {
        let owner = step.kind.owner();
        let items = match &step.kind {
            StepKind::Kubernetes(objects) => match cluster.as_mut() {
                Some(Ok(cluster)) => cluster.plan(kubernetes::declared(inputs, objects)).await,
                Some(Err(reason)) => vec![Item::unread("Kubernetes", reason)],
                None => unreachable!("a Kubernetes step connects"),
            },
            StepKind::Helm(release) => helm::plan(inputs, release).await,
            StepKind::Hub(declared) => {
                let gateway = gateway.as_ref().and_then(|gateway| gateway.as_ref().ok());
                hub::plan(inputs, declared, gateway).await
            }
            kind => match gateway.as_ref() {
                Some(Ok(gateway)) => plan_openshell(inputs, gateway, kind).await,
                Some(Err(reason)) => vec![Item::unread("OpenShell", reason)],
                None => unreachable!("an OpenShell step connects"),
            },
        };
        planned.push(StepPlan::new(&step.name, owner, items));
    }
    Ok(Plan { steps: planned })
}

async fn plan_openshell(inputs: &Inputs, gateway: &Gateway, kind: &StepKind) -> Vec<Item> {
    match kind {
        StepKind::Workspaces(workspaces) => gateway.plan_workspaces(workspaces).await,
        StepKind::Profiles(profiles) => gateway.plan_profiles(inputs, profiles).await,
        StepKind::Templates(templates) => gateway.plan_templates(inputs, templates).await,
        StepKind::Sandboxes(sandboxes) => gateway.plan_sandboxes(inputs, sandboxes).await,
        _ => unreachable!("only OpenShell steps come here"),
    }
}
