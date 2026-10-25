# veoveo-installer

The operator's command that makes a cluster match this repository: `plan` shows what
differs, `apply` changes exactly that. Its design is in
[Applying An Installation](../DESIGN.md#applying-an-installation); the steps it applies
are [installation.toml](../installation.toml).

## Build and check

```sh
cd installer
cargo build --release            # target/release/veoveo-installer
```

The one check runs from the repository root, in Docker:

```sh
docker build -f installer/Dockerfile --target check .
docker build -f installer/Dockerfile --target binary --output type=local,dest=out .
```

## Inputs

`--repository` is this repository (default `.`, so run it from the repository root).
`--private` (or `VEOVEO_INSTALLER_PRIVATE`) is the operator's private directory, mode
`0700`, holding `private.toml`; [private.example.toml](private.example.toml) shows every
key. Every command but the fixtures needs it.

## plan

```sh
veoveo-installer --private ~/private plan [--step <name>]...
```

Reads every declared piece from its owner and lists, in step order, what differs,
what could not be read and what apply would do. It changes nothing. It exits `0` when
everything was read and matches, `1` when it cannot plan at all, such as with an
unreadable `private.toml`, and `2` otherwise. What it deliberately does not compare,
such as a seed Job, counts as matching.

## apply

```sh
veoveo-installer --private ~/private apply [--step <name>]...
```

Plans, then makes the plan's changes in step order, checks each and plans each changed
step again; it stops at the first failure with its evidence and exits `1`. It changes
nothing unless the plan read every piece and has a change for every difference. Run
again, it finds nothing to change. `--step`, repeatable, limits plan or apply to the
named steps of `installation.toml`.

## login

Reading or publishing Hub's control plane needs the operator's own Hub sign-in, as an
installation administrator:

```sh
veoveo-installer --private ~/private login [--no-browser]
```

It prints the sign-in address and opens a browser, and waits up to five minutes for Hub
to send the browser back to `http://127.0.0.1:3211/callback`, where it listens.
`--no-browser` only prints the address. When the installer runs on another machine,
such as over SSH, forward that port from the machine whose browser signs in, run
`login --no-browser` there and open the printed address in that browser:

```sh
ssh -L 3211:127.0.0.1:3211 operator@install-host
```

## identity hub

```sh
veoveo-installer --private ~/private identity hub [--check]
```

Prepares Hub's trusted files (`hub.env`, `authorization-server.jwks.json`, `hub.json`)
in the private directory's `[identity] directory`, from the identity provider's
registration in `private.toml`. Keys are made once; over existing files it verifies
them and refuses rather than replace one. `--check` verifies and says what it would
write, and writes nothing.

## Mac fixtures

`fixture keycloak`, `fixture openshell`, `fixture hub` and `fixture public-client`
prepare the Mac development fixtures: local Keycloak, the native OpenShell gateway's
OIDC clients, the fixture Hub's catalog and keys, and a local PKCE client. No Linux
installation uses them.
