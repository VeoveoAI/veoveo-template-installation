This private Computer provides OpenCode, the configured Hub MCP connection and
native Lore CLI. Hub supplies authorized platform tools and branch selection;
Lore owns source repositories, native login, authorization and refresh.

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
Read that public file when offering the personal repository. An empty Lore
repository list does not determine whether a known assigned repository can be cloned.
`VEOVEO_LORE_REMOTE` is the installation's private Lore URI, not a native setting.
When a chosen operation needs first login, use native Lore:

```sh
lore auth login "$VEOVEO_LORE_REMOTE" --no-browser
```

The user completes the native prompt; Lore stores and refreshes its own grant.
Clone with the chosen repository URI and destination, then use that checkout's
configured remote. Source reaches remote followers only after explicit commit
and push, not file saves or local commits alone.

## Served branches

A branch of a server repository that has `openshell/run` is served once it is
pushed to Lore: Hub builds it and runs it in its own sandbox. Local edits and local
commits serve nothing. Hub's tools report each branch's status and logs and select
which branch of each server this account uses; a server without a selection uses
the installation's base branch. A selection applies to every chat of this account.

## Artifact transfers

Use Hub's authorized Artifact operations to publish outputs or request downloads.
Transfer bytes with native `/usr/bin/curl`, preserving the exact grant method,
signed URL and required headers. Do not rewrite/redirect the request or mix Hub
or model authorization into it. Keep signed capabilities private: use a mode0600
curl config under `/sandbox` (`umask 077`), run `/usr/bin/curl --config`, then remove
it; avoid verbose/trace output. Ask Hub for a fresh operation if the grant expires.
Complete any required upload/finalize step; byte transfer alone is not publication.
Record safe operation/Artifact IDs, status, byte counts and hashes, not signed URLs.
