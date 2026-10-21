This private Computer provides OpenCode, the configured Hub MCP connection and
native Lore CLI. Hub supplies authorized platform tools and environment routing;
Lore owns source repositories, native login, authorization and refresh. Factories
are separate agent-free build/check jobs, not the Computer's coding agent or a
mandatory image-publication step for every edit.

## Workspace and publication

`/sandbox` persists across ordinary Computer stop/start; deletion/reset can discard
it. `HOME=/sandbox/home`, with persistent XDG config/data/state/cache beneath it.
Lore's native auth store is `/sandbox/home/.config/lore/auth`; ordinary OpenCode
shell commands inherit these paths. Keep credential stores outside source snapshots.
Use `/sandbox/projects/<chosen-name>` for projects or optional native checkouts;
preserve existing user layout and branch choices. Keep disposable work separate,
for example `/sandbox/scratch`; `/tmp` is temporary and caches are rebuildable.
Clean only your own identified scratch, preserving user changes and other sessions.
Private workspace persistence does not publish source or outputs. No home sync,
automatic commit/push or automatic output upload is performed.

Hub may prepare an optional personal repository default and write public
coordinates to `/sandbox/home/.config/computer/workspace.env`. It does not log in
or clone. The user or agent chooses repository, branch and destination when wanted.
`VEOVEO_LORE_REMOTE` is the installation's private Lore URI, not a native setting.
When a chosen operation needs first login, use native Lore:

```sh
lore auth login "$VEOVEO_LORE_REMOTE" --no-browser
```

The user completes the native prompt; Lore stores and refreshes its own grant.
Clone with the chosen repository URI and destination, then use that checkout's
configured remote. Source reaches remote followers only after explicit commit
and push, not file saves or local commits alone.

## Environments and source updates

Without explicit selection, Hub MCP uses the profile's configured default
environment. Checkout alone never selects or applies an environment. Select an
admitted environment with the `veoveo-environment` HTTP header on the ordinary
Hub MCP connection, retaining its configured URL and native opaque authentication.
OpenCode project/Location configuration scopes that connection; separate chats in
one Location do not independently select it. Routing does not clone data or add ACLs.

Apply or explicitly watch a chosen source directory using the same definition:

```sh
veoveo-environment apply --definition environment.json --local time=./time --exclude .git --exclude .lore --exclude target
veoveo-environment watch --definition environment.json --local time=./time --exclude .git --exclude .lore --exclude target
```

Replace `time=./time` with the actual owned component and chosen path. The client
uses existing `COMPUTER_HUB_URL`/`COMPUTER_HUB_REFERENCE`; do not copy the reference
into source. Exclusions are explicit, not automatic; keep credentials, scratch and
unrelated outputs outside the selected source. Apply uploads a private source
snapshot and updates desired runtime source using admitted runtime/storage inputs.
Watch repeats for changed selected source/definitions, not the home. Neither
command commits/pushes Lore or selects the application's MCP connection.

A `--local` source apply replaces that component's Lore-follow selection; local
definition apply also replaces definition-file following. Lore follows only explicitly
selected published branches; pins stay exact. A failed candidate retains any
previous serving worker; an override without one is unavailable, not inherited.
Use apply status/build diagnostics to distinguish desired from serving source.
Runtime services continue independently of the Computer; its local watch stops
when the Computer stops.

## Artifact transfers

Use Hub's authorized Artifact operations to publish outputs or request downloads.
Transfer bytes with native `/usr/bin/curl`, preserving the exact grant method,
signed URL and required headers. Do not rewrite/redirect the request or mix Hub
or model authorization into it. Keep signed capabilities private: use a mode0600
curl config under `/sandbox` (`umask 077`), run `/usr/bin/curl --config`, then remove
it; avoid verbose/trace output. Ask Hub for a fresh operation if the grant expires.
Complete any required upload/finalize step; byte transfer alone is not publication.
Record safe operation/Artifact IDs, status, byte counts and hashes, not signed URLs.
