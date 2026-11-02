---
name: served-branches
description: How a pushed branch of a server repository is built, served and selected, and how to tell why one does not serve.
---

# Served branches

A branch of a server repository that has `openshell/run` is served once it is
pushed to Lore: Hub builds it and runs it in its own sandbox. Local edits and local
commits serve nothing. Hub's tools report each branch's status and logs and select
which branch of each server this account uses; a server without a selection uses
the installation's base branch. A selection applies to every chat of this account.
A server's slug names its schema, permissions and tools, and belongs to one repository:
a repository started from a template gives its server a slug of its own before its first
push, as the template's README says. Hub refuses a server whose slug another repository's
server already serves, and the branch's logs say so.
