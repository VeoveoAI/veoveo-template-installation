---
name: lore
description: "Clone, change, commit and push Lore repositories with the Lore CLI; create one; bring a Git checkout into Lore."
---

# Lore repositories

`VEOVEO_LORE_REMOTE` is the installation's private Lore URI, not a native setting.
Lore is already signed in as this Computer's owner: the owner's Hub sign-in is the
only login, so do not run `lore auth login`. Every `lore` command acts as the owner
on each repository the owner can reach: their own, and those shared with them or with a
group they belong to. A share or a membership change while the Computer runs reaches
it within five minutes.
`LORE_ACCESS_TOKEN` holds only a placeholder that the sandbox's proxy replaces in
Lore's requests; never copy it elsewhere.
To start a new repository, run `lore repository create "$VEOVEO_LORE_REMOTE/<name>"` in
the directory that becomes its workspace: the owner owns it at once, so the next `lore`
command can push it.
The owner or agent chooses the repository, branch and destination. An empty Lore
repository list does not determine whether a known assigned repository can be
cloned. `$VEOVEO_LORE_REMOTE` is a Lore endpoint, not a Git remote. Clone a repository
from it with `lore clone "$VEOVEO_LORE_REMOTE/<repository ID>" <destination>`, adding
`--branch <branch>` for another branch; `lore repository list` shows each ID. In a Lore
checkout, `lore stage --scan .` stages new and changed files (`lore status` lists only
what is staged or tracked), `lore commit "<message>"` records them and `lore push`
publishes them; `lore sync` updates the checkout.
Git works as usual for other sources. To bring a Git checkout into Lore, first add
`.git/` to a `.loreignore` file at its top, because Lore leaves out only what that file
lists and an ignore rule does not remove what is already committed; then run
`lore repository create "$VEOVEO_LORE_REMOTE/<name>"` in it.
Source reaches remote followers only after explicit commit and push, not file saves
or local commits alone.
