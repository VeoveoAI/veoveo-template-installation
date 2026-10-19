# Private native Artifact

Run from the installation root after the qualified [Hub + Time setup](HUB_TIME.md).
This adds private `vv-services/artifact`, port 8801, using the existing Hub, PostgreSQL
and SeaweedFS. It does not expose a public Artifact/share route, migrate storage,
reset schemas or add a launcher. The execution checklist records actual qualification;
source templates and successful builds alone prove no deployed behavior.

## Trusted preparation

Select the exact reviewed Artifact runtime image ID and matching test artifact.
The runtime runs as UID 10001. Ordinary restarts retain the approved service key,
`artifact` schema and object data. An authorized development cutover instead resets
disposable application state and starts current migrations on a fresh database;
legacy key adoption, history migrations and backups are not prerequisites. Retain
trusted installation inputs and use the [fresh reference preparation](REFERENCE.md#fresh-bootstrap-and-retired-reference-keys)
when replacing a retired installation. Never mount an operator/application
directory wholesale. Artifact obtains its own PostgreSQL login through Hub
registration and migrates only its own schema.

Use the existing SeaweedFS 4.48 native identity/IAM interfaces to provision a dedicated
service signing credential scoped to bucket `artifacts`. The existing `.env` storage
credential belongs to trusted bootstrap administration; do not copy it into
Artifact. Before activation verify that the bootstrap administrator remains usable,
the service can perform the bucket/object/multipart operations Artifact requires,
and it is refused access to an unrelated bucket and administrative identity APIs.
Ordinary reruns preserve existing identities and data. A SeaweedFS development
reset also removes its IAM state: reprovision the dedicated Artifact identity and
bucket policy before starting the fresh service. Credential provisioning is a
trusted operator operation, not part of `configure-hub` or an agent factory.

SeaweedFS 4.48's `s3.user.provision -role=readwrite` preset alone is insufficient:
Artifact also initializes the bucket/lifecycle and manages multipart uploads.
After provisioning, inspect the service user's attached policy name and replace
that policy using the native `weed shell` IAM command and the reviewed
[Artifact storage policy](artifact-storage-policy.json). For the existing user
`vv-artifact` provisioned with that preset, the generated name is normally
`artifacts-vv-artifact-readwrite`; verify the actual attachment before updating it.
Make the JSON available to the trusted process running `weed shell` (for a
container shell, copy only this nonsecret file into the container first), then run
these commands in that shell with the verified name and file path:

```text
s3.policy -get -name=artifacts-vv-artifact-readwrite
s3.policy -put -name=artifacts-vv-artifact-readwrite -file=/tmp/artifact-storage-policy.json
s3.policy -get -name=artifacts-vv-artifact-readwrite
```

This updates the attached policy without replacing the user's signing credentials.
Its bucket-only actions are `s3:CreateBucket`, `s3:ListBucket` and
`s3:PutLifecycleConfiguration` on `arn:aws:s3:::artifacts`. Its object actions are
`s3:GetObject`, `s3:PutObject`, `s3:DeleteObject`, `s3:CreateMultipartUpload`,
`s3:UploadPart`, `s3:CompleteMultipartUpload`, `s3:ListMultipartUploadParts` and
`s3:AbortMultipartUpload` on `arn:aws:s3:::artifacts/*`. Startup calls CreateBucket
even when the bucket exists; an existing bucket does not remove that authorization
requirement. If Artifact's `cors_origins` is nonempty, add only
`s3:PutBucketCors` to the bucket statement in the reviewed installation copy;
SeaweedFS 4.48 uses that exact spelling. The default policy omits it.

No bare `Admin`, IAM administration, bucket deletion, unrelated bucket access or
blanket action grant is required. Read back the applied policy, then verify startup,
small signed PUT/GET, multipart completion/abort, unrelated-bucket refusal and
administrative identity-API refusal using the service credential. Inspect protocol
status/error codes; a generic CLI error alone does not prove access was denied.
Keep the bootstrap administrator separate and verify it remains usable.

Store only the service's standard environment in ignored
`state/openshell/artifact/storage.env`, directory 0700/file 0600:

```sh
VEOVEO_STORAGE_ENDPOINT=http://host.openshell.internal:9000
VEOVEO_STORAGE_SIGNING_ENDPOINT=http://host.openshell.internal:9000
VEOVEO_STORAGE_REGION=us-east-1
VEOVEO_STORAGE_BUCKET=artifacts
# VEOVEO_STORAGE_ACCESS_KEY and VEOVEO_STORAGE_SECRET_KEY are filled privately.
```

Both endpoints must name the exact address reachable by Artifact and every signed
URL client. Artifact uses path-style S3 addressing. SigV4 requires the real signing
key inside the trusted service; an opaque reference cannot replace it. Never put
that key on command arguments, in an image, an acceptance runner or a factory.
Native TCP admission on 9000 preserves signed authority/headers and carries bytes
directly to S3; Hub carries metadata/authorization only. This local HTTP storage
listener remains the existing loopback-published SeaweedFS endpoint, not public
storage ingress.

For clients running directly on this Mac, configure the same reserved hostname in
the Mac's hosts file as well:

```text
127.0.0.1 host.openshell.internal
```

OpenShell already resolves this name through its native host-loopback backend for
sandbox workloads; that resolution does not configure the Mac's resolver. Adding
the local hosts entry requires macOS administrator authentication. Preserve existing
entries and reject a conflicting address. Check ordinary hostname resolution and a
signed GET from the Mac after configuration, then check the unchanged URL from the
native acceptance sandbox. A `curl --resolve` override is not client qualification.
Do not replace the URL hostname with `localhost`, change its Host header or add a
download proxy: the signed authority must remain intact. This is a local installation
choice; other installations must supply their own shared reachable S3 authority.

Add the explicit static native binding with the existing trusted helper:

```sh
./identity/configure-hub \
  --hub-origin https://vv-control--hub--http.openshell.localhost:17670 \
  --include-artifact
```

The generated `hub.json` retains Time and adds only
`http://artifact.internal:8801/artifact/mcp` → workspace `vv-services`, sandbox
`artifact`, port 8801. A later ordinary helper rerun preserves the selected Artifact
binding; conflicting operator placement fails for review. The helper retains Hub
keys, registration secret and dedicated IdP identities. Copy the updated `hub.json`
privately into the existing `vv-hub-config` volume using the established trusted
preparation. Hub captures native bindings at startup: a native Hub stop/start is
required to activate this additional placement. Verify the same external exposure,
public key and Time route afterward. No live catalog rebind or dynamic service
composition is implied.

## Provider and volumes

In a trusted subshell, import the standard registration profile and store Hub's
initial registration token only in native credential custody:

```sh
(
  set -a
  . ./state/openshell/hub/hub.env
  set +a
  openshell profile import --workspace vv-services --file openshell/providers/artifact-registration.yaml
  openshell provider create --workspace vv-services --name vv-artifact-registration \
    --type vv-artifact-registration --credential VEOVEO_REGISTRATION_SECRET
)
```

Inspect existing definitions before creation; preserve matching state and refuse
foreign/conflicting definitions. Artifact receives only its opaque registration
reference, bound to POST `/oauth/register`. SDK metadata/JWKS and registration
management have separate narrow REST rules in [its policy](artifact.yaml).
The supervisor's workload CA supplies verified HTTPS trust; no TLS bypass or
caller/provider secrets are added to the Artifact image.

Use only workspace-labelled named volumes. The already approved native driver
configuration keeps resource admission enabled and host binds disabled; the driver
switch also admits validated tmpfs/CDI fields, but these specs request neither.

```sh
docker volume create --label openshell.ai/sandbox-attachable=true \
  --label openshell.ai/sandbox-attachable-workspace=vv-services vv-artifact-state
docker volume create --label openshell.ai/sandbox-attachable=true \
  --label openshell.ai/sandbox-attachable-workspace=vv-services vv-artifact-config
```

First inspect existing volumes, labels and contents; never overwrite foreign state.
Trusted preparation copies only the adopted `keys/server-key.p8` into
`vv-artifact-state/keys`, preserving mode 0600 and the same key. The state directory
is writable by 10001 and survives sandbox replacement. Populate `vv-artifact-config`
with only the dedicated `storage.env`, directory 0700/file 0600 readable by 10001.
[Mount configuration](artifact-mounts.json) makes service state read-write and the
credential configuration read-only. No whole operator directory or Docker socket is
mounted.

## Start and qualify

Start only after PostgreSQL, SeaweedFS and the updated Hub are ready. Supply the
exact approved immutable image ID; inspect an existing same-name sandbox before
creation rather than replacing unknown state:

```sh
openshell sandbox create --workspace vv-services --name artifact \
  --from "$ARTIFACT_RUNTIME_IMAGE_ID" --policy openshell/artifact.yaml \
  --provider vv-artifact-registration --no-auto-providers \
  --driver-config-json "$(cat openshell/artifact-mounts.json)" \
  --restart-policy on-failure --detach \
  --env VEOVEO_HUB_ISSUER=https://vv-control--hub--http.openshell.localhost:17670 \
  --env VEOVEO_PUBLIC_BASE_URL=http://artifact.internal:8801 \
  --env VEOVEO_PORT=8801 --env VEOVEO_KEY_DIRECTORY=/var/lib/veoveo/artifact/keys -- \
  /bin/sh -c 'set -a; . /etc/veoveo/artifact/storage.env; set +a; exec /usr/local/bin/veoveo-mcp-artifact'
```

Keep `VEOVEO_PUBLIC_ORIGIN` unset. Artifact's logical Host/path and forwarded JWT
audience remain `http://artifact.internal:8801/artifact/mcp`; native ForwardTcp
reaches its private loopback listener. It needs no `--allow-loopback-hosts` flag
because this path retains the logical authority. `/readyz` is private and includes
bucket availability; readiness alone does not prove registration/discovery/access.

Prepare a separately owned trusted acceptance runner using
[artifact-qualification.yaml](artifact-qualification.yaml) and the existing
[dual-credential profile](providers/hub-qualification.yaml). Import/configure its
member/admin refresh material only in that runner's workspace as in HUB_TIME.md.
The executable is `/sandbox/bin/artifact-acceptance`. Both handles are distinct;
no S3 signing credential is mounted in the runner. The runner's direct TCP 9000
rule admits only that executable's signed byte transfers. MCP 2026-07-28 inspection
explicitly admits Artifact's named tools, resources, subscriptions and Task control.
The risky member probes must reach Hub and return MCP/tool refusals; a native
transport failure is no permission-refusal evidence.

```sh
export VEOVEO_ACCEPTANCE_HUB=https://vv-control--hub--http.openshell.localhost:17670
export VEOVEO_ACCEPTANCE_MEMBER_BEARER_ENV=HUB_MEMBER_REFERENCE
export VEOVEO_ACCEPTANCE_ADMIN_BEARER_ENV=HUB_ADMIN_REFERENCE
/sandbox/bin/artifact-acceptance --ignored --skip restart_persistence_ \
  --skip a_share_link_redeems_through_the_share_route_until_its_limit \
  --nocapture --test-threads=1
```

Run these six private cases serially. They exercise signed upload/download integrity
and ranges, verification/sweep, Hub permission denial, grant listing/revocation,
quota and access-request approval. The current one-tenant/two-contributor setup
does not prove cross-tenant or object isolation through deployment; those remain
component tests. Public share redemption remains a retained, separately qualified
seventh case.

Run `restart_persistence_prepare` alone, retain its printed standard Artifact ID
and availability timestamp, then have the trusted operator prove native Artifact
and Hub stop/start plus readiness with the same state volume and public signing-key
thumbprint. Run `restart_persistence_verify` alone with those domain inputs. It
requires a fresh subscription, unchanged metadata/access and exact retained bytes,
then removes its test artifact. See the [domain-owned instructions](https://github.com/VeoveoAI/veoveo-mcp-artifact/blob/main/README.md#restart-persistence).
The test never controls lifecycle or receives management credentials.

Observe applied native provider renewal before a restart probe and leave a measured
margin before the next update. Renewal intentionally fences old connections; do
not hide an interrupted operation or replay a mutation. Record actual image IDs,
registration endpoint, public key, native lifecycle and successful/denied calls.
No in-flight verification-Task recovery, public sharing, Media integration or new
customer/tenant issuer qualification is claimed by this slice.
