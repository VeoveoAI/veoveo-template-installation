//! `apply`: makes exactly the plan's changes, step by step in the declaration's order,
//! each through its owner. It changes nothing unless the plan read every piece and has a
//! change for every difference. After a step's changes it plans the step again and stops
//! unless the step then matches; it stops at the first failure with its evidence. Run
//! again, it finds nothing to change.

use std::{collections::BTreeSet, time::Instant};

use anyhow::{Context, Result, anyhow, bail};

use crate::{
    action::Action,
    declaration::StepKind,
    helm, hub,
    inputs::Inputs,
    kubernetes::{self, ROLLOUT_TIMEOUT},
    plan::{Item, State, StepPlan},
    run::{self, Connections},
};

/// How long Hub may take to serve after it starts.
const SERVE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);

/// Applies the steps named in `only`, or every step; returns how many changes it made.
pub async fn apply(inputs: &Inputs, only: &[String]) -> Result<usize> {
    let (plan, mut connections) = run::plan(inputs, only).await?;
    print!("{}", plan.render());
    let blockers = plan.blockers();
    if !blockers.is_empty() {
        let list: Vec<String> = blockers
            .iter()
            .map(|(step, subject, reason)| format!("  {step}: {subject}: {reason}"))
            .collect();
        bail!(
            "apply changes nothing, since the plan is not complete:\n{}",
            list.join("\n")
        );
    }
    let changes: usize = plan
        .steps
        .iter()
        .map(|step| {
            step.items
                .iter()
                .filter(|item| item.action.is_some())
                .count()
        })
        .sum();
    if changes == 0 {
        println!("Nothing to change.");
        return Ok(0);
    }
    let mut made = 0;
    let mut hub_dependents = false;
    for step in &plan.steps {
        if step.items.iter().all(|item| item.action.is_none()) {
            continue;
        }
        let declared = inputs
            .declaration
            .step(&step.name)
            .context("a planned step is declared")?;
        hub_dependents |= awaits_hub_dependents(inputs, declared);
        let mut applied = BTreeSet::new();
        let mut restart = BTreeSet::new();
        for item in &step.items {
            let Some(action) = &item.action else { continue };
            let started = Instant::now();
            println!(
                "{}. {}: {}: {}",
                step.position,
                step.name,
                item.subject,
                action.describe()
            );
            execute(inputs, &mut connections, action)
                .await
                .map_err(|error| anyhow!("{}: {}: {error:#}", step.name, item.subject))?;
            println!("   done in {:.1} s", started.elapsed().as_secs_f64());
            made += 1;
            if let Action::ApplyObject {
                object,
                restart: workloads,
                ..
            } = action
            {
                applied.insert(kubernetes::subject(object));
                restart.extend(workloads.iter().cloned());
            }
        }
        // A workload that read a changed object at its start, and was not itself changed.
        for workload in restart.difference(&applied) {
            let started = Instant::now();
            println!(
                "{}. {}: restart {workload}, which reads a changed object at its start",
                step.position, step.name
            );
            connections
                .cluster()
                .map_err(|reason| anyhow!(reason))?
                .restart(workload)
                .await
                .map_err(|error| anyhow!("{}: {workload}: {error:#}", step.name))?;
            println!("   done in {:.1} s", started.elapsed().as_secs_f64());
        }
        if let Ok(cluster) = connections.cluster() {
            cluster.forget();
        }
        let again = run::plan_step(inputs, step.position, declared, &mut connections).await;
        check_matches(&again)?;
    }
    if hub_dependents {
        await_after_hub(inputs, &mut connections).await?;
    }
    println!("Applied {made} change(s).");
    Ok(made)
}

/// After a step's changes, everything it declares matches.
pub fn check_matches(step: &StepPlan) -> Result<()> {
    let left: Vec<&Item> = step
        .items
        .iter()
        .filter(|item| {
            matches!(
                item.state,
                State::Absent { .. } | State::Differs { .. } | State::Unread(_) | State::Refused(_)
            )
        })
        .collect();
    if left.is_empty() {
        return Ok(());
    }
    let mut evidence = StepPlan::new(step.position, &step.name, step.owner, Vec::new());
    evidence.items = left
        .into_iter()
        .map(|item| Item::new(item.subject.clone(), clone_state(&item.state)))
        .collect();
    let rendered = crate::plan::Plan {
        steps: vec![evidence],
    }
    .render();
    bail!(
        "after its changes, step {} still does not match:\n{rendered}",
        step.name
    )
}

fn clone_state(state: &State) -> State {
    match state {
        State::Same => State::Same,
        State::Absent { note } => State::Absent { note: note.clone() },
        State::Differs { differences, note } => State::Differs {
            differences: differences.clone(),
            note: note.clone(),
        },
        State::Refused(reason) => State::Refused(reason.clone()),
        State::Undeclared(note) => State::Undeclared(note.clone()),
        State::Unread(reason) => State::Unread(reason.clone()),
        State::NotCompared(reason) => State::NotCompared(reason.clone()),
    }
}

async fn execute(inputs: &Inputs, connections: &mut Connections, action: &Action) -> Result<()> {
    match action {
        Action::ApplyObject { object, remove, .. } => {
            connections
                .cluster()
                .map_err(|reason| anyhow!(reason))?
                .apply_object(object, remove)
                .await
        }
        Action::UpgradeRelease { release, values } => helm::upgrade(inputs, release, values).await,
        Action::CreateWorkspace(name) => gateway(connections)?.create_workspace(name).await,
        Action::ImportProfile {
            workspace,
            profile,
            source,
        } => {
            gateway(connections)?
                .put_profile(workspace.as_deref(), profile, source, None)
                .await
        }
        Action::UpdateProfile {
            workspace,
            profile,
            source,
            resource_version,
        } => {
            gateway(connections)?
                .put_profile(
                    workspace.as_deref(),
                    profile,
                    source,
                    Some(*resource_version),
                )
                .await
        }
        Action::ReplaceTemplate {
            workspace,
            template,
            exists,
        } => {
            gateway(connections)?
                .replace_template(workspace, template, *exists)
                .await
        }
        Action::ReplaceSandbox {
            workspace,
            name,
            request,
            services,
            exists,
        } => {
            gateway(connections)?
                .replace_sandbox(workspace, name, request, services, *exists)
                .await?;
            after_sandbox(inputs, workspace, name).await
        }
        Action::ExposeService {
            workspace,
            sandbox,
            service,
            ..
        } => {
            gateway(connections)?
                .ensure_service(workspace, sandbox, service)
                .await
        }
        Action::WriteClaimFiles { claim, files } => {
            connections
                .cluster()
                .map_err(|reason| anyhow!(reason))?
                .write_claim(inputs, claim, files)
                .await
        }
        Action::Publish {
            origin,
            profile,
            declared,
            hub: (workspace, name),
        } => {
            let published = hub::publish(inputs, origin, profile, declared).await?;
            println!("   revision {} is active", published.revision);
            if published.read_at_start() {
                println!(
                    "   {workspace}/{name} reads a changed member only at its start; restarting it"
                );
                gateway(connections)?
                    .restart_sandbox(workspace, name)
                    .await?;
                hub::await_serving(origin, SERVE_TIMEOUT).await?;
            }
            Ok(())
        }
        Action::AwaitRegistration {
            origin,
            profile,
            server,
            deployment,
        } => hub::await_registration(inputs, origin, profile, server, deployment).await,
        Action::RemoveClaimFiles { claim, names } => {
            if names.is_empty() {
                return Ok(());
            }
            connections
                .cluster()
                .map_err(|reason| anyhow!(reason))?
                .remove_from_claim(inputs, claim, names)
                .await
        }
    }
}

fn gateway(connections: &Connections) -> Result<&crate::openshell::Gateway> {
    connections.gateway().map_err(|reason| anyhow!(reason))
}

/// The checks after a sandbox is created: Hub serves its issuer and the Computer
/// version, and a server registers with Hub as healthy.
async fn after_sandbox(inputs: &Inputs, workspace: &str, name: &str) -> Result<()> {
    let Ok((origin, hub)) = hub::declared_origin(inputs) else {
        return Ok(());
    };
    if hub.workspace == workspace && hub.sandbox == name {
        return hub::await_serving(&origin, SERVE_TIMEOUT).await;
    }
    let key = format!("{workspace}/{name}");
    let Some((_, server, deployment)) = hub::registering(inputs)
        .into_iter()
        .find(|(sandbox, _, _)| *sandbox == key)
    else {
        return Ok(());
    };
    match hub::Administrator::signed_in(inputs, &origin, &hub.profile) {
        Ok(_) => hub::await_registration(inputs, &origin, &hub.profile, &server, &deployment).await,
        Err(error) => {
            println!("   its registration with Hub is not checked: {error:#}");
            Ok(())
        }
    }
}

/// Whether, once `step` has changed, apply waits after its last step for the workloads
/// that become ready only once Hub serves: the step changes what Hub serves, or holds
/// such a workload. Connections::open connects Kubernetes for every such step.
pub fn awaits_hub_dependents(inputs: &Inputs, step: &crate::declaration::Step) -> bool {
    run::changes_hub(inputs, step)
        || matches!(&step.kind, StepKind::Kubernetes(objects) if !objects.after_hub.is_empty())
}

/// The workloads that become ready only once Hub serves, checked after the last step
/// when a changed step awaits them.
async fn await_after_hub(inputs: &Inputs, connections: &mut Connections) -> Result<()> {
    let workloads: Vec<String> = inputs
        .declaration
        .steps
        .iter()
        .flat_map(|step| match &step.kind {
            StepKind::Kubernetes(objects) => objects.after_hub.clone(),
            _ => Vec::new(),
        })
        .collect();
    for workload in workloads {
        let object = kubernetes::workload_object(&workload)?;
        connections
            .cluster()
            .map_err(|reason| anyhow!(reason))?
            .await_ready(&object, ROLLOUT_TIMEOUT)
            .await
            .map_err(|error| {
                anyhow!("{workload}, which becomes ready once Hub serves: {error:#}")
            })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::difference::compare;

    #[test]
    fn a_step_that_still_differs_after_its_changes_stops_apply_with_its_evidence() {
        let matching = StepPlan::new(
            9,
            "templates",
            "OpenShell",
            vec![
                Item::compared("template installation/factory-v1", vec![], None),
                Item::new("seed", State::NotCompared("a seed Job".into())),
            ],
        );
        assert!(check_matches(&matching).is_ok());
        let differing = StepPlan::new(
            9,
            "templates",
            "OpenShell",
            vec![Item::compared(
                "template installation/computer-v1",
                compare(
                    &json!({"image": "computer@sha256:2"}),
                    &json!({"image": "computer@sha256:1"}),
                ),
                None,
            )],
        );
        let error = check_matches(&differing).unwrap_err().to_string();
        assert!(
            error.contains("step templates still does not match"),
            "{error}"
        );
        assert!(
            error.contains("image: declared \"computer@sha256:2\", active \"computer@sha256:1\""),
            "{error}"
        );
    }
}
