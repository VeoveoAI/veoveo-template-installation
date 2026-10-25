//! Planning every declared step in order, each through its owner's interface.

use anyhow::{Result, bail};

use crate::{
    declaration::{Step, StepKind},
    helm, hub,
    inputs::Inputs,
    kubernetes::{self, Cluster},
    openshell::{self, Gateway},
    plan::{Item, Plan, StepPlan},
};

/// The owners a run reaches, connected once; a failed connection keeps its reason.
pub struct Connections {
    cluster: Option<Result<Cluster, String>>,
    gateway: Option<Result<Gateway, String>>,
}

/// Which owners' connections a step needs, in plan and in apply, whatever its owner.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Needs {
    pub cluster: bool,
    pub gateway: bool,
}

impl Needs {
    /// What `step` reaches. A step that changes what Hub serves also needs Kubernetes,
    /// where apply then waits for the workloads that become ready once Hub serves.
    pub fn of(inputs: &Inputs, step: &Step) -> Self {
        let mut needs = match &step.kind {
            StepKind::Kubernetes(_) => Self {
                cluster: true,
                gateway: false,
            },
            StepKind::Helm(_) => Self::default(),
            // Read in Hub's sandbox; written by the claim's seed Job.
            StepKind::Claim(_) => Self {
                cluster: true,
                gateway: true,
            },
            // Hub's sandbox and restart through OpenShell; the claim's cleanup Job.
            StepKind::Hub(_) => Self {
                cluster: true,
                gateway: true,
            },
            _ => Self {
                cluster: false,
                gateway: true,
            },
        };
        if changes_hub(inputs, step) {
            needs.cluster = true;
        }
        needs
    }

    /// What the steps together reach.
    pub fn of_steps(inputs: &Inputs, steps: &[&Step]) -> Self {
        steps.iter().fold(Self::default(), |all, step| {
            let needs = Self::of(inputs, step);
            Self {
                cluster: all.cluster || needs.cluster,
                gateway: all.gateway || needs.gateway,
            }
        })
    }
}

/// Whether applying `step` can change what Hub serves: Hub's trusted claim, Hub's
/// control plane, or a step that holds Hub's sandbox. After such a step apply waits for
/// the workloads that become ready once Hub serves.
pub fn changes_hub(inputs: &Inputs, step: &Step) -> bool {
    let hub = inputs
        .declaration
        .steps
        .iter()
        .find_map(|step| match &step.kind {
            StepKind::Hub(hub) => Some(hub),
            _ => None,
        });
    match &step.kind {
        StepKind::Claim(_) | StepKind::Hub(_) => true,
        StepKind::Sandboxes(sandboxes) => hub.is_some_and(|hub| {
            sandboxes
                .iter()
                .any(|sandbox| sandbox.workspace == hub.workspace && sandbox.name == hub.sandbox)
        }),
        _ => false,
    }
}

impl Connections {
    /// Connects to the owners the steps need.
    pub async fn open(inputs: &Inputs, steps: &[&Step]) -> Self {
        let needs = Needs::of_steps(inputs, steps);
        let cluster = if needs.cluster {
            Some(
                kubernetes::connect(inputs.private.kubernetes.context.as_deref())
                    .await
                    .map(Cluster::new)
                    .map_err(|error| format!("{error:#}")),
            )
        } else {
            None
        };
        let gateway = if needs.gateway {
            Some(match &inputs.private.openshell {
                Some(access) => openshell::connect(access)
                    .await
                    .map_err(|error| format!("{error:#}")),
                None => Err(
                    "private.toml names no OpenShell gateway sign-in ([openshell] gateway)"
                        .to_owned(),
                ),
            })
        } else {
            None
        };
        Self { cluster, gateway }
    }

    pub fn cluster(&mut self) -> Result<&mut Cluster, String> {
        match self.cluster.as_mut() {
            Some(Ok(cluster)) => Ok(cluster),
            Some(Err(reason)) => Err(reason.clone()),
            None => Err("Kubernetes was not connected".to_owned()),
        }
    }

    pub fn gateway(&self) -> Result<&Gateway, String> {
        match self.gateway.as_ref() {
            Some(Ok(gateway)) => Ok(gateway),
            Some(Err(reason)) => Err(reason.clone()),
            None => Err("the OpenShell gateway was not connected".to_owned()),
        }
    }
}

/// The steps named in `only`, or every step, with their positions from 1.
pub fn selected<'a>(inputs: &'a Inputs, only: &[String]) -> Result<Vec<(usize, &'a Step)>> {
    if let Some(unknown) = only
        .iter()
        .find(|name| inputs.declaration.step(name).is_none())
    {
        bail!("installation.toml declares no step {unknown}");
    }
    Ok(inputs
        .declaration
        .steps
        .iter()
        .enumerate()
        .filter(|(_, step)| only.is_empty() || only.contains(&step.name))
        .map(|(position, step)| (position + 1, step))
        .collect())
}

/// Plans the steps named in `only`, or every step. It changes nothing.
pub async fn plan(inputs: &Inputs, only: &[String]) -> Result<(Plan, Connections)> {
    let steps = selected(inputs, only)?;
    let refs: Vec<&Step> = steps.iter().map(|(_, step)| *step).collect();
    let mut connections = Connections::open(inputs, &refs).await;
    let mut planned = Vec::with_capacity(steps.len());
    for (position, step) in steps {
        planned.push(plan_step(inputs, position, step, &mut connections).await);
    }
    Ok((Plan { steps: planned }, connections))
}

/// Plans one step.
pub async fn plan_step(
    inputs: &Inputs,
    position: usize,
    step: &Step,
    connections: &mut Connections,
) -> StepPlan {
    let owner = step.kind.owner();
    let items = match &step.kind {
        StepKind::Kubernetes(objects) => match connections.cluster() {
            Ok(cluster) => {
                cluster
                    .plan(kubernetes::declared(inputs, objects), &objects.restart)
                    .await
            }
            Err(reason) => vec![Item::unread("Kubernetes", reason)],
        },
        StepKind::Helm(release) => helm::plan(inputs, release).await,
        StepKind::Claim(claim) => match connections.gateway() {
            Ok(gateway) => hub::plan_claim(inputs, claim, gateway).await,
            Err(reason) => vec![Item::unread("OpenShell", reason)],
        },
        StepKind::Hub(declared) => hub::plan(inputs, declared, connections.gateway().ok()).await,
        kind => match connections.gateway() {
            Ok(gateway) => plan_openshell(inputs, gateway, kind).await,
            Err(reason) => vec![Item::unread("OpenShell", reason)],
        },
    };
    StepPlan::new(position, &step.name, owner, items)
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
