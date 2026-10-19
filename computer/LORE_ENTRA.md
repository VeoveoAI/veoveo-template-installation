# Native human Lore access with Microsoft Entra

This is trusted installation configuration for the native Lore direct-role mode.
It is not a receipt of Entra login, signed claims, role removal or refresh acceptance.
The native source is reviewed at `b32265924ccf1d3c09de8f76ea4eecc8c2562b05` after focused native
regressions and CLI/server compilation. Build CLI and server together from that
source, then rebuild the Computer with the resulting Lore image before using
[lore-entra.example.toml](lore-entra.example.toml). These image builds and actual
Entra acceptance remain pending; earlier `821da217` built-image receipts retain
their historical scope. The existing
[structured/exchange fragment](lore-oidc.example.toml) remains a separate option for
an issuer that supports that native contract.

Use the same existing Lore S3/PostgreSQL storage and private TLS endpoint described
in [LORE_STORAGE.md](../openshell/LORE_STORAGE.md). No storage reset, credential
broker, signer or additional identity/authorization service is needed. Keep storage
credentials solely in the trusted server. Review existing factory clients/issuer
and hook assignments before selecting a coherent human auth profile; replacing
one issuer's profile must not silently invalidate or broaden another workload's
rights. Do not advertise two unrelated issuers as a discovery workaround.

## API and public CLI registration

Create/select two distinct single-tenant application registrations through native
Microsoft Graph or the Entra admin center:

1. The **Lore resource API** uses `signInAudience = "AzureADMyOrg"`,
   `api.requestedAccessTokenVersion = 2`, and a delegated API scope such as
   `api://<Lore-API-application-id>/access_as_user`. Define user-assignable app roles
   on this API (`allowedMemberTypes = ["User"]`), with literal values matching the
   native mappings, such as `Lore.ProjectExample.Member`. Use fresh role GUIDs for
   new definitions. Preserve existing role IDs, scopes, values and assignments.
2. The separate **public CLI** enables native public-client/device login, has no
   client secret and requests only the delegated Lore API scope plus the native
   identity/offline scopes. Native Lore adds `offline_access` to device login.
   Grant the required delegated consent; requesting the scope does not assign a
   repository role. No Microsoft Graph application permission is needed at runtime.
3. Assign each intended user or approved group only its exact API role(s), through
   the API enterprise application's app-role assignments. Do not assign native
   OpenShell platform-admin roles, factory identities or a wildcard repository role.
   Native API and CLI application IDs are distinct from Hub and OpenShell APIs.

Microsoft documents [API roles in access tokens](https://learn.microsoft.com/en-us/entra/identity-platform/howto-add-app-roles-in-apps#usage-scenario-of-app-roles)
and [the public-client device grant](https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-device-code).
Role values have a 120-character limit. Roles and delegated scopes share a default
700-definition limit; this exact-repository mapping is for a bounded installation,
not an unlimited-resource authorization design. See
[appRole](https://learn.microsoft.com/en-us/graph/api/resources/approle?view=graph-rest-1.0)
and [role limits](https://learn.microsoft.com/en-us/entra/identity-platform/howto-add-app-roles-in-apps#app-role-limits).

## Signed recipient binding

Entra v2 API access tokens use the API application GUID as `aud`. That GUID alone
cannot satisfy native Lore's signed destination-host check. Keep the check intact:
configure the Lore API's resource service principal with an ordinary Claims Mapping
Policy that emits a **signed scalar** `root_domains` claim for the actual private
Lore DNS name. Native Lore already accepts a string or string array. The hostname
must be the real TLS-verified Computer-to-Lore destination, for example
`lore.veoveo.example.com`, not an alias for the issuer or Hub.

The native Graph create-policy body is:

```json
{
  "definition": [
    "{\"ClaimsMappingPolicy\":{\"Version\":1,\"IncludeBasicClaimSet\":\"true\",\"ClaimsSchema\":[{\"Value\":\"lore.veoveo.example.com\",\"JwtClaimType\":\"root_domains\"}]}}"
  ],
  "displayName": "Lore API signed recipient",
  "isOrganizationDefault": false
}
```

Use `POST /v1.0/policies/claimsMappingPolicies`, then associate the returned policy
with the **Lore API resource service principal**, not the public CLI or Hub, using
`POST /v1.0/servicePrincipals/<Lore-API-service-principal-object-id>/claimsMappingPolicies/$ref`
with the native `@odata.id` reference. Review an existing mapping policy before
changing it: claims mapping supersedes other claims customization, and one active
mapping must preserve required identity/role behavior. Do not replace an unrelated
policy or append incompatible definitions. On this single-tenant API application,
set `api.acceptMappedClaims = true` and keep `requestedAccessTokenVersion = 2`,
preserving its other API properties. Do not use this setting for an unreviewed
multitenant application or introduce a new signing key. See Microsoft's
[static claim schema](https://learn.microsoft.com/en-us/entra/identity-platform/reference-claims-customization#claim-schema-entry-elements)
and [resource policy/acceptMappedClaims procedure](https://learn.microsoft.com/en-us/graph/how-to-claims-customization).

The public CLI's access token, including refreshed access tokens, must actually
contain this signed recipient claim and the assigned roles. An ID-token claim or
an environment value is not a substitute. Entra access tokens normally carry
`typ = "JWT"`; the example admits that specific API profile and exact tenant
issuer/API audience. Its separate public-client ID makes an ID token addressed
to the CLI unsuitable for the API. Do not disable type, audience, signature or
recipient validation to make a probe pass.

## Native grants and acceptance

Merge the selected fragment into trusted server configuration without replacing
TLS, backend storage or hook configuration. Direct mode is mutually exclusive
with `resource_claim`, `permission_claim`, `[environment.endpoint] auth_url` and
OIDC exchange settings; remove those from this selected profile rather than
leaving a mixed-mode configuration. Match each role to one real native repository
ID. `permissions = []` is ordinary repository access, including unprotected writes,
not read-only access. Privileged native actions require explicit approved mapping;
no mapping sets `is_service_account` or bypasses protected branches or creator checks.
Unknown roles grant nothing and malformed role arrays cannot partially authorize.
`token_grants` lists existing repositories using the same current grants.

Review the real tenant discovery/device/token authorities and admit exact native
Computer egress for those authorities and the private Lore endpoint. The generic
policy's `identity.veoveo.example.com` placeholder is not an Entra hostname;
select the actual `login.microsoftonline.com` tenant flow and any documented,
actually required device-login authorities. Preserve end-to-end TLS. Do not widen
the Hub-managed authority or use a hostname alias to evade native endpoint binding.

Before selecting this profile for users, prove with native operations and separate
ordinary identities: browser-approved device login; expected issuer, audience,
`typ`, recipient and exact roles; native authorized repository listing/clone/sync;
denial of an unrelated repository and a protected-branch write without losing
permitted inspection; exact pushed revision/bytes; refresh and concurrent rotation;
and role removal reflected in newly issued credentials. Keep tokens, device codes,
refresh material and signed capabilities out of logs/reports. A role removal does
not retroactively revoke an already-issued JWT or strengthen existing long-lived
native session semantics. Factory explicit-token behavior remains a separate gate.
