---
name: computer
description: "Use whenever work depends on this private Computer: what it provides, what persists in the home, the Hub CLI, and how files move through signed URLs."
---

# This Computer

This private Computer provides OpenCode, the configured Hub MCP connection, the
native Lore CLI and the Hub CLI `veoveo-hub-sdk-cli`. Hub supplies authorized platform
tools, branch selection and the owner's Lore access; Lore owns source repositories and
their authorization.

From a shell or a script, `veoveo-hub-sdk-cli tools` lists the same Hub tools as the
owner and `veoveo-hub-sdk-cli call <tool> '<JSON arguments>'` calls one, waiting for a
Task-backed tool to end; `resources`, `prompts` and `read <uri>` work the same way and
`--json` prints JSON, with a result's `_meta`, such as the Task a write started, which
`veoveo-hub-sdk-cli tasks get <task-id>` follows. It needs no flags or sign-in: the proxy
supplies the owner's Hub token. Its `login`, `logout` and `ask` do not apply here.

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

## Signed transfers

A signed URL a tool returns, to upload or download bytes, is a credential. Here only
`/usr/bin/curl` may send it to the public host's `/artifacts/` path; OpenCode's own
`fetch` and other programs are refused. Send it with exactly its method, URL and
headers, without rewriting or redirecting it or adding other authorization. Keep it
private: write it to a mode 0600 curl config under `~` (`umask 077`), run
`/usr/bin/curl --config <file>`, then delete the file, and use no verbose or trace
output. Record IDs, byte counts and hashes, never the URL. When a signed URL expires, ask the
tool for a new one.

## Version

`COMPUTER_VERSION` names the version this Computer started at. A program here reads its
status, the current version and what an update brings with `GET /computer/status` on the
Hub's origin (`COMPUTER_HUB_URL` without its path) and
`Authorization: Bearer $COMPUTER_HUB_REFERENCE`; the proxy supplies the owner's token.
The person's browser shows it at `/computer/status`.
