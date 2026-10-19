# Build prepared Time source

This is trusted installation preparation, followed by a bounded deterministic job. Execute
publishing and preflight only after the [execution checklist](../../veoveo-docs/OPENSHELL_HARDCUT_PLAN.md)
accepts deployed Hub/Time. The source, image, providers and release build must pass
their component checks first. Agents work on Computers and publish source to Lore;
the factory clones the selected immutable revision and runs the declared commands.
These instructions do not record runtime acceptance.
Keep the original `factory.yaml` and disposable Lore qualification identities intact.

## Tool image

Use Time's existing Dockerfile `dev` image through a verified local tag,
`TIME_DEV_IMAGE_REF`, and retain its approved immutable `TIME_DEV_IMAGE_ID`. A raw
`sha256:` image ID is not a valid Docker `FROM` reference in this setup. Use the
same Docker engine/builder that owns the verified local tag. It supplies the
component toolchain; the shared layer supplies Lore, installation CA and the SDK's
Cargo-installed CLI. SDK
`a1d9d653ae3d723821cdeeab92f4a67acdca39ce` is the committed installation contract;
there is no SDK runtime image. The layer contains no prebuilt Time release binary.
Run from this installation root after the existing Lore client image build:

```sh
(
  set -eu
  TIME_DEV_IMAGE_REF=veoveo/time-dev:native-time-prep
  TIME_DEV_IMAGE_ID=sha256:6382e247f3b0c077dafa5bd5a28418663847d0be55247f230684849f03c6b396
  test "$(docker image inspect --format '{{.Id}}' "$TIME_DEV_IMAGE_REF")" = "$TIME_DEV_IMAGE_ID"
  docker build -f factory/Dockerfile --build-arg BASE="$TIME_DEV_IMAGE_REF" \
    --build-context lore=docker-image://veoveo/lore-client:local \
    --build-context ca=./tls/ca -t veoveo/factory-time:local factory
  test "$(docker image inspect --format '{{.Id}}' "$TIME_DEV_IMAGE_REF")" = "$TIME_DEV_IMAGE_ID"
  docker image inspect --format '{{.Id}}' veoveo/factory-time:local
)
```

The checks before and after the build must both match the approved base ID. Record
the resulting factory image ID as `FACTORY_TIME_IMAGE_ID`; native sandbox `--from`
uses that exact `sha256:` ID for publisher, preflight and build job. The reviewed
Time commit `90eabf9d3a103075c2aff907f008cf63f341f822` is published on its Git remote
and is the prepared source for `TIME_GIT_COMMIT`. The existing
[component lock](../component-lock.json) records the published reviewed component
heads. A later source change requires new component checks and a reviewed published
commit before selecting another tree; do not add a source-sync manifest.

## Trusted source publisher

Choose and retain canonical 32-character lowercase hexadecimal `TIME_REPOSITORY_ID`
and `TIME_BRANCH_ID`, a dedicated client ID (here `vv-lore-time`), and the exact
prepared Git commit. Use a new ordinary repository, never the fixture repositories.
The helper's named mode does not need a fixture file or modify fixture clients:

```sh
./identity/configure-lore --client-id vv-lore-time \
  --repository-id "$TIME_REPOSITORY_ID"
```

It configures one optional `lore-repo-ID` grant, `at+jwt`, the Lore audience and
ordinary stable subject. It preserves an existing client secret, enabled state,
role mappings and trusted assignments; mismatched saved identity/secret fails.
It never adds Lore's `is_service_account` bypass. Default fixture provisioning
remains separate and preserves the Time entry. Files remain under the existing
0700 `state/openshell/lore-direct/` directory with mode 0600:

- `config.json` retains the existing clients and adds `clients.time`.
- `time.secret` and `time.refresh.env` are private trusted bootstrap material.
- `time.provider.yaml` is a standard native profile rendered from
  [lore-time.example.yaml](providers/lore-time.example.yaml) with the exact repo scope.
- `server.toml` retains all other trusted hook assignments and server settings.

Lore's current `--access-token` and login `--token` accept literal arguments, with
no supported environment, stdin or token-file reader. `LORE_AUTH_PATH` selects its
encrypted store directory; it is not a raw-token input. Never put a real bearer on
host argv or manufacture that store. Use a disposable **trusted native publisher**
with opaque references on argv instead. Its operator-owned workspace has only the
new repository's provider and [publisher-time.yaml](publisher-time.yaml), no
inference, Hub/admin credentials, host binds or Docker access. Ordinary CLI calls
create/publish the repository; no offline seeder, human login, issuer or daemon is
required. Native v1 repository creation is not an existing-repository authorization
check: the exact ID and subsequent grant use must be verified during publishing.

Refuse an already existing `vv-time-source` workspace or `publisher` sandbox;
record their ownership before creating resources. Inspect uncertain create outcomes
and clean up only resources proven to belong to this operation. Do not replace
unrelated resources. Create the workspace through the trusted native operator:

```sh
openshell workspace create vv-time-source
(
  set -a
  . ./state/openshell/lore-direct/time.refresh.env
  set +a
  openshell profile import --workspace vv-time-source \
    --file state/openshell/lore-direct/time.provider.yaml
  openshell provider create --workspace vv-time-source --name vv-lore-time \
    --type vv-lore-time --runtime-credentials
  openshell provider refresh configure --workspace vv-time-source vv-lore-time \
    --credential-key LORE_AUTH_TOKEN --strategy oauth2-client-credentials \
    --material "client_id=$VV_LORE_TIME_CLIENT_ID" \
    --secret-material-env client_secret=VV_LORE_TIME_CLIENT_SECRET
  openshell provider refresh rotate --workspace vv-time-source vv-lore-time \
    --credential-key LORE_AUTH_TOKEN
)
openshell sandbox create --workspace vv-time-source --name publisher \
  --from "$FACTORY_TIME_IMAGE_ID" --policy openshell/publisher-time.yaml \
  --provider vv-lore-time --no-auto-providers --detach -- sleep infinity
```

This initial rotation is setup, not renewal evidence. `LORE_AUTH_TOKEN` inside the
sandbox is an identity-bound opaque reference. Read only the nonsecret `subject`
from `config.json.clients.time` into `TIME_SUBJECT`. No client secret/JWT is passed
through Exec environment options, argv or output.

Export the exact tested Git tree, not the dirty working tree. This native Lore
path does not track symlinks or Git submodules. Reject modes `120000` and `160000`
in the selected Git tree before publishing; do not flatten or silently omit them.
Keep the archive under ignored private operator state. Check the export against
Git's tracked paths, bytes and executable bits, including any export attributes.
Keep Git and Lore working directories separate. Run this block in Bash:

```bash
(
  set -euo pipefail
  git -C ../veoveo-mcp-time ls-tree -r "$TIME_GIT_COMMIT" | awk '
    $1 == "120000" || $1 == "160000" {
      print "Unsupported source entry: " $0 > "/dev/stderr"
      rejected = 1
    }
    END { exit rejected }
  '
  umask 077
  git -c tar.umask=0022 -C ../veoveo-mcp-time archive --format=tar "$TIME_GIT_COMMIT" \
    > state/openshell/lore-direct/time-source.tar
  openshell sandbox upload --workspace vv-time-source --no-git-ignore \
    publisher state/openshell/lore-direct/time-source.tar /sandbox/time-source.tar
)
```

Use ordinary native Exec, a bounded timeout and the normal Lore CLI. The following
first-publication commands intentionally fail on an existing local directory or
repository. If any command fails, inspect the exact remote/local state before
continuing; do not rerun initial publishing blindly:

```sh
openshell sandbox exec --workspace vv-time-source --no-tty --no-login-shell \
  --timeout 600 --env "TIME_REPOSITORY_ID=$TIME_REPOSITORY_ID" \
  --env "TIME_BRANCH_ID=$TIME_BRANCH_ID" --env "TIME_SUBJECT=$TIME_SUBJECT" \
  --env "TIME_GIT_COMMIT=$TIME_GIT_COMMIT" publisher -- bash -euc '
  mkdir /sandbox/time-source
  cd /sandbox/time-source
  lore -P --identity "$TIME_SUBJECT" --access-token "$LORE_AUTH_TOKEN" \
    repository create --id "$TIME_REPOSITORY_ID" grpcs://10.231.10.10:41337/time
  tar -xf /sandbox/time-source.tar
  lore -P --identity "$TIME_SUBJECT" --access-token "$LORE_AUTH_TOKEN" stage --scan .
  lore -P --identity "$TIME_SUBJECT" --access-token "$LORE_AUTH_TOKEN" \
    revision commit "Prepared Git $TIME_GIT_COMMIT"
  lore -P --identity "$TIME_SUBJECT" --access-token "$LORE_AUTH_TOKEN" branch push main
  lore -P --identity "$TIME_SUBJECT" --access-token "$LORE_AUTH_TOKEN" branch protect main
  lore -P --identity "$TIME_SUBJECT" --access-token "$LORE_AUTH_TOKEN" \
    branch create factory-time --id "$TIME_BRANCH_ID"
  lore -P --identity "$TIME_SUBJECT" --access-token "$LORE_AUTH_TOKEN" branch info main
  lore -P --identity "$TIME_SUBJECT" --access-token "$LORE_AUTH_TOKEN" branch info factory-time
'
```

The tracked Time `.loreignore` is already part of the source. Do not add metadata,
another ignore file or build output merely to seed Lore. Verify the remote repo ID,
protected main, factory branch ID and immutable revision. A branch requires an
initial committed revision; this is why assignment follows first publication.
Then finalize trusted assignment explicitly:

```sh
./identity/configure-lore --client-id vv-lore-time \
  --repository-id "$TIME_REPOSITORY_ID" --assigned-branch-id "$TIME_BRANCH_ID"
```

The helper atomically replaces `state/openshell/lore-direct/server.toml`. The
existing Compose service **`vv-lore`** mounts that single file read-only at
`/etc/lore/config/local.toml`; Lore constructs its hook dispatcher at startup.
Recreate that container to bind the new file and load the finalized assignment:

```sh
docker compose up -d --no-deps --no-build --force-recreate vv-lore
```

Use the existing approved Lore image. This operation preserves the external
`vv-local_vv-lore` data volume, identity configuration and unrelated services;
never remove the data volume as part of activation. Verify the replacement is
ready and its mounted configuration matches the reviewed file byte-for-byte.
Verify that `factory_branch` is enabled, the authenticated Time subject maps to
exactly `TIME_REPOSITORY_ID` / `TIME_BRANCH_ID`, and ordinary Lore branch inspection
still reports protected main and the selected factory branch. Complete these
checks before creating or releasing factory credentials. A rerun without
`--assigned-branch-id` retains the finalized assignment. The hook is an
ordinary-push workflow guard, not comprehensive low-level branch isolation.

Clone the selected full Lore revision into a fresh directory and compare it with
the Git export: exact paths, file types, bytes and executable bits; confirm no
symlinks or submodules were introduced. Exclude only VCS metadata, retaining the
tracked `.loreignore` and all other source dotfiles. Use standard native
upload/download/Exec operations; do not add a synchronization service or persist a
new correspondence format. Record Git commit and Lore revision in the existing
execution checklist. Only then delete the owned publisher sandbox/provider/profile/
workspace; preserve the Lore repository, identity client, protected branches, hook
assignment and durable server data.

## Factory credentials and policy

Only after the assignment/correspondence review, create a fresh operator-approved
`vv-time-build` workspace. Import the rendered Time profile and repeat the native
Lore provider create/configure/initial-rotate commands above with that workspace.
Keep each workspace's native custody separate. Attach the production **member**
profile [hub-member.yaml](providers/hub-member.yaml), using its existing trusted
client material and native refresh procedure from [Hub/Time setup](HUB_TIME.md).
Do not attach its admin, qualification, management or registration providers.

Inspect existing objects before reruns; do not replace foreign/conflicting state.
The factory policy is [factory-time.yaml](factory-time.yaml). It permits only Lore
read RPCs, sparse registry/download GETs, Git read protocol paths for the three
pinned Cargo dependency repositories and member SDK discovery/`time__resolve_time`.
The SDK CLI is the preflight client. Git fetches use Cargo's CLI mode; no Git push
endpoint, host Git credentials, token fetcher, rustup downloads or unbounded network
rule is included.
Verify actual executable paths, redirects and dependency access during preflight;
report unsupported operations to development rather than widening the policy.

```sh
openshell sandbox create --workspace vv-time-build --name factory-time \
  --from "$FACTORY_TIME_IMAGE_ID" --policy openshell/factory-time.yaml \
  --provider vv-lore-time --provider vv-hub-member \
  --no-auto-providers --detach -- sleep infinity
```

Use UID 1000 and `/sandbox` for writable state, with no service UID-10001 volume or
operator mounts. Installed toolchains and CLI binaries remain read-only. Use the
provisioned workload CA through existing native `SSL_CERT_FILE`; never disable TLS
verification or rewrite Host. The exact Hub HTTPS authority must match the actual
native exposure; the policy's local origin is not a substitute for that check.

## Preflight, then one bounded build job

With the exact image/policy/providers, manually clone `TIME_LORE_REVISION` from
`grpcs://10.231.10.10:41337/TIME_REPOSITORY_ID` using the same `--identity` and opaque
`--access-token` inputs, followed by `repository clone URL PATH --revision HASH`.
Set `CARGO_HOME=/sandbox/cargo` and `CARGO_NET_GIT_FETCH_WITH_CLI=true` **after** any
login-shell profile. Keep the installed `RUSTUP_HOME=/usr/local/rustup`. Use a fresh
`CARGO_TARGET_DIR=/sandbox/time-preflight-target`; Time already has SQLX_OFFLINE.
Run exactly:

```sh
cargo build --locked --release --bin veoveo-mcp-time
veoveo-hub-sdk-cli --hub https://vv-control--hub--http.openshell.localhost:17670 \
  --bearer-env HUB_MEMBER_REFERENCE --json tools
veoveo-hub-sdk-cli --hub https://vv-control--hub--http.openshell.localhost:17670 \
  --bearer-env HUB_MEMBER_REFERENCE --json call time__resolve_time \
  '{"expression":{"format":"rfc3339","value":"2026-10-17T12:00:00Z"}}'
```

Check tool discovery and typed successful Time output, not only HTTP status. Verify
writable Cargo state, dependency downloads, release binary and unchanged source.
Failure returns diagnostics to the user's Computer for a source fix and a new
published revision.

After preflight, run the exact clone/build command through native Exec with a
finite 900-second deadline. The trusted caller supplies the admitted immutable
revision and identity; the sandbox receives its existing opaque Lore reference.
Fresh source and target directories distinguish this job's output from preflight:

```sh
openshell sandbox exec --workspace vv-time-build --no-tty --no-login-shell \
  --timeout 900 --env "TIME_REPOSITORY_ID=$TIME_REPOSITORY_ID" \
  --env "TIME_LORE_REVISION=$TIME_LORE_REVISION" --env "TIME_SUBJECT=$TIME_SUBJECT" \
  factory-time -- bash -euc '
  test ! -e /sandbox/time-job
  test ! -e /sandbox/time-job-target
  export CARGO_HOME=/sandbox/cargo
  export CARGO_NET_GIT_FETCH_WITH_CLI=true
  export RUSTUP_HOME=/usr/local/rustup
  export CARGO_TARGET_DIR=/sandbox/time-job-target
  lore -P --identity "$TIME_SUBJECT" --access-token "$LORE_AUTH_TOKEN" \
    repository clone "grpcs://10.231.10.10:41337/$TIME_REPOSITORY_ID" \
    /sandbox/time-job --revision "$TIME_LORE_REVISION"
  cd /sandbox/time-job
  cargo build --locked --release --bin veoveo-mcp-time
  sha256sum /sandbox/time-job-target/release/veoveo-mcp-time
'
```

Retain the admitted revision, image identity, command, exit status, diagnostics and
binary digest. Verify unchanged source. The trusted publisher builds the runtime
image from that same verified Lore revision using Time's existing Dockerfile;
native deployment, readiness and a Hub-routed call remain separate execution gates.
The factory receives neither deployment nor registry credentials. Model-driven
qualification runs separately on a Computer after the deterministic gates pass.

For the runtime image, export the verified Lore source into a clean temporary
Docker build context, excluding `.lore` metadata. Time's current `.dockerignore`
excludes `.git`, but does not exclude `.lore`. Compare the exported source to the
prepared tree before building the existing `runtime` target.

Native image replacement requires recreating only the owned `vv-services/time`
sandbox: confirm its identity and state mount, stop/delete it, await confirmed
absence, then repeat the [Time create command](HUB_TIME.md) with the new immutable
image. Keep its name, policy, registration provider, environment and `vv-time-state`
volume. Do not delete the volume, provider or workspace. Hub's unchanged native
binding resolves the sandbox name for each new connection, so it requires no
restart. Verify the actual image, private readiness, retained public key and a
fresh Hub-routed member call; new sandbox/process/registration client IDs are expected.
