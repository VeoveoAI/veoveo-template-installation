This private Computer provides OpenCode, the configured Hub MCP connection, the
native Lore CLI and the Hub CLI `veoveo-hub-sdk-cli`. Hub supplies authorized platform
tools, branch selection and the owner's Lore access; Lore owns source repositories and
their authorization.

From a shell or a script, `veoveo-hub-sdk-cli tools` lists the same Hub tools as the
owner and `veoveo-hub-sdk-cli call <tool> '<JSON arguments>'` calls one, waiting for a
Task-backed tool to end; `resources`, `prompts` and `read <uri>` work the same way and
`--json` prints JSON. It needs no flags or sign-in: the proxy supplies the owner's Hub
token. Its `login`, `logout` and `ask` do not apply here.

## Home and publication

`~` is always kept; everything else comes with the Computer version. `~` is this
Computer's home and belongs to its owner: programs keep their own configuration,
data and caches there, and the platform writes nothing into it. A restart loses
only running programs and `/tmp`. Keep credential stores out of source checkouts.
Put projects and checkouts where the owner wants them under `~`, and preserve the
existing layout and branch choices. Keep disposable work separate, for example
`~/scratch`; `/tmp` is temporary and caches are rebuildable. Clean only your own
identified scratch, preserving user changes and other sessions. Keeping `~`
publishes nothing: there is no home sync, automatic commit/push or automatic
output upload.

`VEOVEO_LORE_REMOTE` is the installation's private Lore URI, not a native setting.
Lore is already signed in as this Computer's owner: the owner's Hub sign-in is the
only login, so do not run `lore auth login`. Every `lore` command acts as the owner
on each repository the owner can reach: their own, and those shared with them or with a
group they belong to. A share or a membership change while the Computer runs reaches
it within five minutes. To share a repository, use `console__share_repository`; to
manage groups, `console__create_group` and `console__set_group_member`.
`LORE_ACCESS_TOKEN` holds only a placeholder that the sandbox's proxy replaces in
Lore's requests; never copy it elsewhere.
To start a new repository, run `lore repository create "$VEOVEO_LORE_REMOTE/<name>"` in
the directory that becomes its workspace, or call the Hub tool
`console__create_repository` with its name and clone it: either way the owner owns it at
once, so the next `lore` command can push or clone it.
The owner or agent chooses the repository, branch and destination. An empty Lore
repository list does not determine whether a known assigned repository can be
cloned. Clone with the chosen repository URI and destination, then use that
checkout's configured remote.
Source reaches remote followers only after explicit commit and push, not file saves
or local commits alone.

## This Computer's version

Call the Hub tool `console__computer` at the start of a session: it reports this
Computer's version, the current one and what an update brings. If an update is
available or was just installed, tell the person what's new in a line or two;
restart only when the person asks, with `restart: true`. A restart keeps `~` and
ends running programs, this session included; the Hub answers before it restarts.
`COMPUTER_VERSION` names the version this Computer started at. A program here reads
the same status with `GET /computer/status` on the Hub's origin (`COMPUTER_HUB_URL`
without its path) and `Authorization: Bearer $COMPUTER_HUB_REFERENCE`; the proxy
supplies the owner's token. The person's browser shows it at `/computer/status`.

## Served branches

A branch of a server repository that has `openshell/run` is served once it is
pushed to Lore: Hub builds it and runs it in its own sandbox. Local edits and local
commits serve nothing. Hub's tools report each branch's status and logs and select
which branch of each server this account uses; a server without a selection uses
the installation's base branch. A selection applies to every chat of this account.
A server's slug names its schema, permissions and tools, and belongs to one repository:
a repository started from a template gives its server a slug of its own before its first
push, as the template's README says. Hub refuses a server whose slug another repository's
server already serves, and the branch's logs say so.

## Artifact transfers

Use Hub's authorized Artifact operations to publish outputs or request downloads.
Transfer bytes with native `/usr/bin/curl`, preserving the exact grant method,
signed URL and required headers. Do not rewrite/redirect the request or mix Hub
or model authorization into it. Keep signed capabilities private: use a mode0600
curl config under `~` (`umask 077`), run `/usr/bin/curl --config`, then remove
it; avoid verbose/trace output. Ask Hub for a fresh operation if the grant expires.
Complete any required upload/finalize step; byte transfer alone is not publication.
Record safe operation/Artifact IDs, status, byte counts and hashes, not signed URLs.
