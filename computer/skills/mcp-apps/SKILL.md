---
name: mcp-apps
description: "How to write the HTML view of an MCP App a server ships: what the Hub's sandbox allows and how the view talks to its server."
---

# MCP Apps

An MCP App is a server's interactive view: one HTML document at `ui://<slug>/<page>`,
MIME `text/html;profile=mcp-app`, which the Hub shows in a sandboxed frame. A person
opens it at `https://<host>/apps/<slug>/<page>`. A Rust server declares it in its
`Routes` with `RoutesBuilder::app`, and names the tools the view may call with
`RoutesBuilder::app_tools`. The full contract is the SDK's
[Apps design](https://github.com/VeoveoAI/veoveo-hub-sdk/blob/main/crates/veoveo-hub-sdk/src/apps/DESIGN.md).

## What the sandbox allows

The view runs in an iframe with `sandbox="allow-scripts"` and nothing else, so it has an
opaque origin. Write it for that:

- **No form submission.** The browser blocks it and logs "Blocked form submission ...
  the 'allow-forms' permission is not set". Use `<button type="button">` with a click
  listener. If a `<form>` is wanted for the Enter key, listen for `submit` and call
  `event.preventDefault()` first.
- **No navigation, popups, cookies or storage.** `localStorage`, `sessionStorage`,
  IndexedDB and cookies are unavailable. Keep state in memory, and keep what must last
  on the server, through its tools.
- **Self-contained.** Inline `<script>` and `<style>` run, and images are `data:` URIs. Any
  other origin must be declared, exactly, in the App resource's `_meta.ui.csp`
  (`connectDomains`, `resourceDomains`, `frameDomains`, `baseUriDomains`): an
  installation-owned origin with no wildcard, path or query. An undeclared origin is
  refused by the Content Security Policy.
- **Nothing framed.** A nested frame inherits the same sandbox. A full web application
  that needs cookies runs on its server's app origin, `<slug>.apps.<host>`, through its
  authenticated routes, and the view opens it top-level with `ui/open-link`.

## Talking to the server

The view sends JSON-RPC 2.0 messages to `window.parent` with `postMessage`, and accepts
only messages whose `event.source` is `window.parent`. It never holds a token: every
request runs as the signed-in person through the Hub.

1. Send `ui/initialize` with `protocolVersion: "2026-01-26"`, `appInfo` and
   `appCapabilities.availableDisplayModes`. Apply the answer's `hostContext` (theme,
   `timeZone`, display mode), then send `ui/notifications/initialized`.
2. Load data with `resources/read` on the server's own resources. Change things with
   `tools/call`, naming the tool as the server knows it (`<tool>`, not
   `<slug>__<tool>`). Only tools linked to this view with app visibility are callable.
3. Answer `ping` and `ui/resource-teardown`, and report height with
   `ui/notifications/size-changed`.
4. For live data, send `subscriptions/listen` naming the server's own resources. Read
   again on each `notifications/resources/updated`, and never poll on a timer.
5. A Task-based tool stays a Task inside the view: follow it with `tasks/get`, and stop
   it with `tasks/cancel`.
6. Show failures in the view. A refused tool is an ordinary failed request, so the view
   can degrade to read-only. Numbers larger than a JavaScript number holds exactly come
   from the server as strings.

```html
<input id="text"> <button type="button" id="go">Save</button>
<script>
  const pending = new Map(); let next = 1;
  const request = (method, params) => new Promise((resolve, reject) => {
    const id = next++; pending.set(id, { resolve, reject });
    window.parent.postMessage({ jsonrpc: "2.0", id, method, params }, "*");
  });
  window.addEventListener("message", (event) => {
    if (event.source !== window.parent) return;
    const m = event.data;
    if (m?.id !== undefined && m.method) {            // a request from the host
      window.parent.postMessage({ jsonrpc: "2.0", id: m.id, result: {} }, "*");
      return;
    }
    const call = pending.get(m?.id); if (!call) return;
    pending.delete(m.id);
    m.error ? call.reject(new Error(m.error.message)) : call.resolve(m.result);
  });
  (async () => {
    await request("ui/initialize", { protocolVersion: "2026-01-26",
      appInfo: { name: "example", version: "1.0.0" },
      appCapabilities: { availableDisplayModes: ["inline", "fullscreen"] } });
    window.parent.postMessage({ jsonrpc: "2.0", method: "ui/notifications/initialized" }, "*");
  })();
  document.getElementById("go").addEventListener("click", async () => {
    const text = document.getElementById("text").value;
    const result = await request("tools/call", { name: "save_note", arguments: { text } });
    console.log(result.structuredContent);
  });
</script>
```

## When it does not work

Open the App at its direct URL and read the browser console. "Blocked form submission"
means a form submitted: use buttons, or `preventDefault()`. "Refused to connect" or
"Refused to load" names an origin to declare in `_meta.ui.csp`. "the tool ... is not linked to the App"
means the server's routes do not link that tool to this view: add it with `app_tools`.
