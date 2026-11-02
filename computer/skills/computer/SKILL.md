---
name: computer
description: Use at the start of a session and whenever work depends on this private Computer: what it provides, what persists in the home, the Hub CLI, and checking or applying a Computer update.
---

# This Computer

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
