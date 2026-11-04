---
name: served-branches
description: "What the Hub needs to serve a branch of a server repository: a push to Lore, an executable openshell/run, and a slug no other repository's server uses."
---

# Served branches

A branch of a server repository is served when it is pushed to Lore and has an
executable `openshell/run`: the Hub builds it and runs it in its own sandbox. Local
edits and local commits serve nothing.

A server's slug names its schema, permissions and tools, and belongs to one repository.
The Hub refuses a server whose slug another repository's server already serves, and
that branch's logs say so. A repository started from a template therefore gives its
server a slug of its own before its first push, as the template's README says.
