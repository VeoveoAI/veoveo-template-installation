"""Secret-free local catalog placement regression tests; no identity/runtime calls."""
import importlib.machinery
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SOURCE = Path(__file__).with_name('configure-hub')
loader = importlib.machinery.SourceFileLoader('configure_hub', str(SOURCE))
spec = importlib.util.spec_from_loader(loader.name, loader)
helper = importlib.util.module_from_spec(spec)
loader.exec_module(helper)


class NativePlacementTests(unittest.TestCase):
    def test_artifact_is_explicit_and_preserves_time_and_admission(self):
        baseline = helper.catalog(helper.HUB_ORIGIN)
        selected = helper.catalog(helper.HUB_ORIGIN, include_artifact=True)
        self.assertEqual(baseline['nativeUpstreams']['bindings'], [helper.TIME_BINDING])
        self.assertEqual(selected['nativeUpstreams']['bindings'],
                         [helper.TIME_BINDING, helper.ARTIFACT_BINDING])
        selected['nativeUpstreams']['bindings'].pop()
        self.assertEqual(selected, baseline)

    def test_rerun_keeps_previously_selected_artifact(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(helper, 'STATE', Path(directory)):
            self.assertFalse(helper.select_services(False, False, helper.retained_catalog())[0])
            self.assertTrue(helper.select_services(True, False, helper.retained_catalog())[0])
            path = Path(directory) / 'hub.json'
            path.write_text(json.dumps(helper.catalog(helper.HUB_ORIGIN)))
            self.assertFalse(helper.select_services(False, False, helper.retained_catalog())[0])
            self.assertTrue(helper.select_services(True, False, helper.retained_catalog())[0])
            path.write_text(json.dumps(helper.catalog(helper.HUB_ORIGIN, include_artifact=True)))
            self.assertTrue(helper.select_services(False, False, helper.retained_catalog())[0])

    def test_foreign_or_changed_native_placement_fails_before_regeneration(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(helper, 'STATE', Path(directory)):
            current = helper.catalog(helper.HUB_ORIGIN, include_artifact=True)
            current['nativeUpstreams']['bindings'][1]['sandbox'] = 'unapproved'
            path = Path(directory) / 'hub.json'
            encoded = json.dumps(current)
            path.write_text(encoded)
            with self.assertRaisesRegex(ValueError, 'review operator placement'):
                helper.select_services(True, False, helper.retained_catalog())[0]
            self.assertEqual(path.read_text(), encoded)


    def test_media_requires_artifact_and_retains_every_approved_selection(self):
        with self.assertRaisesRegex(ValueError, 'Media requires selected Artifact'):
            helper.catalog(helper.HUB_ORIGIN, include_media=True)
        selected = helper.catalog(helper.HUB_ORIGIN, include_artifact=True, include_media=True)
        self.assertEqual(selected['nativeUpstreams']['bindings'],
                         [helper.TIME_BINDING, helper.ARTIFACT_BINDING, helper.MEDIA_BINDING])
        self.assertEqual(helper.select_services(False, False, selected), (True, True, False))
        self.assertEqual(helper.catalog(helper.HUB_ORIGIN, retained=selected), selected)
        artifact = helper.catalog(helper.HUB_ORIGIN, include_artifact=True)
        self.assertEqual(helper.select_services(False, True, artifact), (True, True, False))

    def test_media_billed_role_is_narrow_and_member_admission_is_unchanged(self):
        baseline = helper.catalog(helper.HUB_ORIGIN, include_artifact=True)
        selected = helper.catalog(helper.HUB_ORIGIN, include_artifact=True, include_media=True)
        self.assertEqual(selected['authorization']['roles'][:-1], baseline['authorization']['roles'])
        self.assertEqual(selected['authorization']['roles'][-1],
                         {'name': 'billed', 'access': ['read', 'write'], 'risks': ['billed']})
        self.assertEqual(selected['authorization']['grants'][:-1], baseline['authorization']['grants'])
        self.assertEqual(selected['authorization']['grants'][-1],
                         {'role': 'billed', 'subject': {'principal': helper.ISSUER + '#vv-hub-billed'}})
        self.assertEqual(selected['oauthClients'][:-1], baseline['oauthClients'])
        billed = selected['oauthClients'][-1]
        self.assertEqual(billed['id'], 'vv-hub-billed')
        self.assertEqual(billed['authMethods'], ['client_secret_post'])
        self.assertNotIn('clientSecret', billed)
        self.assertEqual(selected['workContexts'][0]['memberships'][0]['oauthClients'],
                         ['vv-hub-member', 'vv-hub-admin', 'vv-hub-billed'])

    def test_media_preserves_context_owner_policy_and_does_not_mutate_retained_input(self):
        baseline = helper.catalog(helper.HUB_ORIGIN, include_artifact=True)
        baseline['workContexts'][0]['outputPolicy']['owner']['id'] = helper.ISSUER + '#retained-owner'
        baseline['workContexts'][0]['policyRevision'] = 'retained'
        baseline['policies'].append({'version': 'retained', 'rules': []})
        before = json.dumps(baseline)
        selected = helper.catalog(helper.HUB_ORIGIN, include_media=True, retained=baseline)
        self.assertEqual(json.dumps(baseline), before)
        self.assertEqual(selected['policies'], baseline['policies'])
        selected['workContexts'][0]['memberships'][0]['oauthClients'].remove('vv-hub-billed')
        self.assertEqual(selected['workContexts'], baseline['workContexts'])

    def test_media_refuses_ambiguous_membership_or_foreign_placement(self):
        retained = helper.catalog(helper.HUB_ORIGIN, include_artifact=True)
        retained['workContexts'][0]['memberships'].append(
            dict(retained['workContexts'][0]['memberships'][0]))
        with self.assertRaisesRegex(ValueError, 'review retained contributor membership'):
            helper.catalog(helper.HUB_ORIGIN, include_media=True, retained=retained)
        for bindings in [
            [helper.TIME_BINDING, helper.MEDIA_BINDING],
            [helper.TIME_BINDING, helper.ARTIFACT_BINDING, dict(helper.MEDIA_BINDING, port=9999)],
            [helper.TIME_BINDING, helper.ARTIFACT_BINDING, helper.MEDIA_BINDING, helper.MEDIA_BINDING],
        ]:
            retained['nativeUpstreams']['bindings'] = bindings
            with self.assertRaisesRegex(ValueError, 'review operator placement'):
                helper.select_services(False, True, retained)

    def test_embedding_is_optional_and_adds_no_authority_or_billed_risk(self):
        baseline = helper.catalog(helper.HUB_ORIGIN)
        selected = helper.catalog(helper.HUB_ORIGIN, include_embedding=True)
        self.assertEqual(selected['nativeUpstreams']['bindings'], [helper.TIME_BINDING, helper.EMBEDDING_BINDING])
        selected['nativeUpstreams']['bindings'].pop()
        self.assertEqual(selected, baseline)

    def test_embedding_retains_every_service_policy_and_grant_without_mutating_input(self):
        for artifact, media in [(False, False), (True, False), (True, True)]:
            baseline = helper.catalog(helper.HUB_ORIGIN, include_artifact=artifact, include_media=media)
            baseline['policies'].append({'version': 'retained', 'rules': []})
            before = json.dumps(baseline)
            selected = helper.catalog(helper.HUB_ORIGIN, retained=baseline, include_embedding=True)
            self.assertEqual(json.dumps(baseline), before)
            self.assertEqual(selected['authorization'], baseline['authorization'])
            self.assertEqual(selected['oauthClients'], baseline['oauthClients'])
            self.assertEqual(selected['workContexts'], baseline['workContexts'])
            self.assertEqual(selected['policies'], baseline['policies'])
            self.assertEqual(selected['nativeUpstreams']['bindings'], baseline['nativeUpstreams']['bindings'] + [helper.EMBEDDING_BINDING])
            self.assertEqual(helper.catalog(helper.HUB_ORIGIN, retained=selected), selected)
            self.assertEqual(helper.select_services(False, False, selected), (artifact, media, True))

    def test_embedding_foreign_duplicate_and_ambiguous_bindings_fail_closed(self):
        selected = helper.catalog(helper.HUB_ORIGIN, include_embedding=True)
        for bindings in [
            [helper.TIME_BINDING, dict(helper.EMBEDDING_BINDING, port=9999)],
            [helper.TIME_BINDING, helper.EMBEDDING_BINDING, helper.EMBEDDING_BINDING],
            [helper.EMBEDDING_BINDING, helper.TIME_BINDING],
            [helper.TIME_BINDING, helper.MEDIA_BINDING, helper.EMBEDDING_BINDING],
        ]:
            selected['nativeUpstreams']['bindings'] = bindings
            with self.assertRaisesRegex(ValueError, 'review operator placement'):
                helper.select_services(False, False, selected)



class LocalPublicClientTests(unittest.TestCase):
    MEMBER = '33333333-3333-4333-8333-333333333333'
    OTHER = '44444444-4444-4444-8444-444444444444'

    def arguments(self):
        return dict(client_id='opencode-local', members=[self.MEMBER], context_id='chosen-work',
                    resource=helper.HUB_ORIGIN + '/mcp/local',
                    redirects=['http://127.0.0.1:3210/callback'])

    def catalog(self):
        retained = helper.catalog(helper.HUB_ORIGIN, include_artifact=True, include_media=True)
        context = retained['workContexts'][0]
        context['id'], context['tenant'] = 'chosen-work', 'chosen-tenant'
        retained['tenants'].append({'id': 'chosen-tenant', 'title': 'Chosen'})
        retained['metadata']['retained'] = 'untouched'
        retained['nativeUpstreams']['bindings'].append({'endpoint': 'http://embedding.internal:8803/mcp',
            'workspace': 'vv-services', 'sandbox': 'embedding', 'port': 8803})
        retained['lore'] = {'retained': 'native contribution data'}
        return retained

    def test_only_explicit_human_member_and_context_are_added_without_rebuilding_catalog(self):
        retained = self.catalog()
        before = json.dumps(retained)
        prepared = helper.local_public_catalog(retained, **self.arguments())
        self.assertEqual(json.dumps(retained), before)
        self.assertEqual(prepared['oauthClients'][-1], {
            'id': 'opencode-local', 'authorizationServer': 'local', 'defaultWorkContext': 'chosen-work',
            'invocationMode': 'direct', 'allowedResources': [helper.HUB_ORIGIN + '/mcp/local'],
            'grantTypes': ['authorization_code_pkce', 'refresh_token'], 'authMethods': ['none'],
            'redirectUris': ['http://127.0.0.1:3210/callback'],
            'allowedScopes': ['mcp', 'offline_access'], 'tenant': 'chosen-tenant', 'metadata': {}})
        self.assertEqual(prepared['authorization']['grants'][-1], {'role': 'member',
            'subject': {'principal': self.MEMBER}, 'tenant': 'chosen-tenant'})
        self.assertEqual(prepared['workContexts'][0]['memberships'][-1],
                         {'level': 'contributor', 'principals': [self.MEMBER]})
        self.assertNotIn(self.OTHER, json.dumps(prepared))
        self.assertEqual(helper.local_public_catalog(prepared, **self.arguments()), prepared)
        prepared['oauthClients'].pop()
        prepared['authorization']['grants'].pop()
        prepared['workContexts'][0]['memberships'].pop()
        self.assertEqual(prepared, retained)

    def test_native_user_properties_preserve_sub_and_have_no_service_or_admin_identity(self):
        prepared = helper.local_public_catalog(self.catalog(), **self.arguments())
        client, mappers = helper.local_public_client(prepared['oauthClients'][-1], prepared['workContexts'][0])
        self.assertTrue(client['publicClient'])
        self.assertTrue(client['standardFlowEnabled'])
        for field in ['serviceAccountsEnabled', 'directAccessGrantsEnabled', 'implicitFlowEnabled', 'fullScopeAllowed']:
            self.assertFalse(client[field])
        self.assertEqual(client['attributes']['pkce.code.challenge.method'], 'S256')
        self.assertNotIn('secret', client)
        self.assertFalse(any(mapper['name'] == 'sub' for mapper in mappers))
        for name in ['principal', 'initiator']:
            mapper, = [mapper for mapper in mappers if mapper['name'] == name]
            self.assertEqual(mapper['protocolMapper'], 'oidc-usermodel-property-mapper')
            self.assertEqual(mapper['config']['user.attribute'], 'id')
            self.assertNotIn('claim.value', mapper['config'])
        self.assertEqual(next(mapper for mapper in mappers
                              if mapper['name'] == 'https://veoveo.ai/claims/work-context')['config']['claim.value'],
                         'chosen-work')

    def test_wrong_context_foreign_issuer_and_conflicting_registration_are_not_replaced(self):
        for changes in [dict(context_id='missing'), dict(members=[]),
                        dict(redirects=['http://elsewhere.example/callback']),
                        dict(redirects=['http://localhost:3210/callback'])]:
            with self.assertRaises(ValueError):
                helper.local_public_catalog(self.catalog(), **dict(self.arguments(), **changes))
        retained = self.catalog()
        retained['authorizationServers'][0]['issuer'] = 'https://foreign.example'
        with self.assertRaisesRegex(ValueError, 'external Keycloak issuer'):
            helper.local_public_catalog(retained, **self.arguments())
        retained = helper.local_public_catalog(self.catalog(), **self.arguments())
        retained['oauthClients'][-1]['allowedScopes'].append('admin')
        with self.assertRaisesRegex(ValueError, 'registration differs'):
            helper.local_public_catalog(retained, **self.arguments())

    def test_native_registration_touches_only_selected_public_client_and_builtin_scopes(self):
        prepared = helper.local_public_catalog(self.catalog(), **self.arguments())
        calls = []
        created = False

        def request(path, method='GET', data=None, token=None):
            nonlocal created
            calls.append((path, method, data))
            base = '/admin/realms/veoveo'
            if path == base + '/clients?clientId=opencode-local':
                return [{'id': 'fixture-client', 'publicClient': True}] if created else []
            if path == base + '/client-scopes':
                return [{'id': name + '-scope', 'name': name} for name in ['basic', 'mcp', 'offline_access']]
            if path == base + '/clients' and method == 'POST':
                created = True
            if method == 'GET':
                return []

        helper.configure_local_public_client(request, 'fixture-operator-token', prepared, 'opencode-local')
        self.assertFalse(any('/users' in path or '/roles' in path or 'client-secret' in path
                             for path, _, _ in calls))
        writes = [(path, method) for path, method, _ in calls if method != 'GET']
        self.assertIn(('/admin/realms/veoveo/clients/fixture-client/optional-client-scopes/offline_access-scope', 'PUT'), writes)
        self.assertIn(('/admin/realms/veoveo/clients/fixture-client/optional-client-scopes/mcp-scope', 'PUT'), writes)
        self.assertTrue(all(path == '/admin/realms/veoveo/clients'
                            or path.startswith('/admin/realms/veoveo/clients/fixture-client/') for path, _ in writes))


class EntraPreparationTests(unittest.TestCase):
    TENANT = '11111111-1111-4111-8111-111111111111'
    CLIENT = '22222222-2222-4222-8222-222222222222'
    MEMBER = '33333333-3333-4333-8333-333333333333'
    ADMIN = '44444444-4444-4444-8444-444444444444'
    BILLED = '55555555-5555-4555-8555-555555555555'
    ORIGIN = 'https://veoveo.example.com'
    ISSUER = 'https://login.microsoftonline.com/' + TENANT + '/v2.0'

    def arguments(self):
        base = 'https://login.microsoftonline.com/' + self.TENANT
        return dict(origin=self.ORIGIN, discovery={'issuer': self.ISSUER,
            'authorization_endpoint': base + '/oauth2/v2.0/authorize',
            'token_endpoint': base + '/oauth2/v2.0/token',
            'jwks_uri': base + '/discovery/v2.0/keys'},
            tenant_id=self.TENANT, client_id=self.CLIENT, tenant='reference', context='reference-work',
            policy='reference-policy', owner=self.ADMIN, members=[self.MEMBER], admins=[self.ADMIN],
            billed=[self.BILLED], application_client='opencode', redirects=['http://127.0.0.1:3210/callback'],
            gateway='https://host.openshell.internal:17670', key_id='public-kid',
            include_artifact=True, include_media=True)

    def test_entra_is_a_hosted_application_broker_with_explicit_authority(self):
        result = helper.entra_catalog(**self.arguments())
        self.assertEqual(result['identityProviders'][0]['claimMapping'],
                         {'subject': 'oid', 'tenant': {'claim': 'tid', 'values': {self.TENANT: 'reference'}}})
        authority = result['authorizationServers'][0]
        self.assertEqual(authority['issuer'], self.ORIGIN + '/oauth')
        self.assertEqual(authority['tokenEndpoint'], self.ORIGIN + '/oauth/token')
        self.assertEqual(result['oidcClients'][0]['redirectUri'], self.ORIGIN + '/oauth/callback')
        caller = result['oauthClients'][0]
        self.assertEqual(caller['authMethods'], ['none'])
        self.assertEqual(caller['grantTypes'], ['authorization_code_pkce', 'refresh_token'])
        self.assertEqual(caller['defaultWorkContext'], 'reference-work')
        self.assertNotIn('credentialSecret', caller)
        self.assertNotIn('jwks', caller)
        self.assertEqual(result['authorization']['grants'][2], {'role': 'billed',
            'subject': {'principal': self.ISSUER + '#' + self.BILLED}, 'tenant': 'reference'})
        self.assertEqual(result['authorization']['roles'][-1]['risks'], ['billed'])
        membership = result['workContexts'][0]['memberships'][0]
        self.assertNotIn('oauthClients', membership)  # Signing in with the public client grants nothing.
        self.assertEqual(set(membership['principals']),
                         {self.ISSUER + '#' + oid for oid in [self.MEMBER, self.ADMIN, self.BILLED]})
        self.assertEqual(result['nativeUpstreams']['bindings'],
                         [helper.TIME_BINDING, helper.ARTIFACT_BINDING, helper.MEDIA_BINDING])
        self.assertNotIn('trustedCertificateAuthorities', result['identityProviders'][0])

    def test_rerun_retains_selected_services_and_policy_without_mutating_input(self):
        arguments = self.arguments()
        retained = helper.entra_catalog(**arguments)
        retained['policies'].append({'version': 'retained-policy', 'rules': []})
        before = json.dumps(retained)
        arguments.update(include_artifact=False, include_media=False, retained=retained)
        self.assertEqual(helper.entra_catalog(**arguments), retained)
        self.assertEqual(json.dumps(retained), before)

    def test_changed_retained_identity_authority_or_placement_is_not_reconciled(self):
        arguments = self.arguments()
        retained = helper.entra_catalog(**arguments)
        for field in ['identityProviders', 'authorization', 'workContexts', 'oidcClients']:
            changed = json.loads(json.dumps(retained))
            changed[field] = []
            with self.assertRaisesRegex(ValueError, 'identity/authority differs'):
                helper.entra_catalog(**arguments, retained=changed)
        changed = json.loads(json.dumps(retained))
        changed['nativeUpstreams']['gateway'] = 'https://unapproved.example.com'
        with self.assertRaisesRegex(ValueError, 'gateway differs'):
            helper.entra_catalog(**arguments, retained=changed)
        changed['nativeUpstreams']['bindings'][0]['sandbox'] = 'foreign'
        with self.assertRaisesRegex(ValueError, 'review operator placement'):
            helper.entra_catalog(**arguments, retained=changed)

    def test_entra_embedding_adds_only_placement_and_retains_identity_authority(self):
        arguments = self.arguments()
        baseline = helper.entra_catalog(**arguments)
        selected = helper.entra_catalog(**arguments, retained=baseline, include_embedding=True)
        self.assertEqual(selected['nativeUpstreams']['bindings'], baseline['nativeUpstreams']['bindings'] + [helper.EMBEDDING_BINDING])
        for field in ['identityProviders', 'authorizationServers', 'profiles', 'authorization',
                      'tenants', 'workContexts', 'oauthClients', 'oidcClients', 'secrets']:
            self.assertEqual(selected[field], baseline[field])
        self.assertEqual(helper.entra_catalog(**arguments, retained=selected), selected)

    def test_bad_urls_tenant_mapping_and_empty_authority_are_rejected(self):
        for origin in ['http://veoveo.example.com', 'https://user@veoveo.example.com',
                       'https://veoveo.example.com/path', 'https://veoveo.example.com/?x=1',
                       'https://veoveo.example.com/#fragment', 'https://veoveo.example.com:99999']:
            with self.assertRaises(ValueError):
                helper.entra_catalog(**dict(self.arguments(), origin=origin))
        for changes in [dict(tenant=''), dict(members=[]), dict(admins=[]), dict(billed=[]),
                        dict(redirects=['http://not-loopback.example.com/callback']),
                        dict(tenant_id=self.CLIENT)]:
            with self.assertRaises(ValueError):
                helper.entra_catalog(**dict(self.arguments(), **changes))
        arguments = self.arguments()
        arguments['discovery']['token_endpoint'] = 'https://foreign.example.com/token'
        with self.assertRaisesRegex(ValueError, 'cross-origin'):
            helper.entra_catalog(**arguments)

    def test_incomplete_retained_state_never_creates_replacement_keys(self):
        import argparse
        with tempfile.TemporaryDirectory() as directory, patch.object(helper, 'STATE', Path(directory)):
            catalog = Path(directory) / 'hub.json'
            catalog.write_text('{}')
            with self.assertRaisesRegex(ValueError, 'incomplete retained'):
                helper.prepare_entra(argparse.Namespace())
            self.assertEqual(sorted(path.name for path in Path(directory).iterdir()), ['hub.json'])
            catalog.unlink()
            private = Path(directory) / 'hub.env'
            private.write_text('retained-private-material')
            with self.assertRaisesRegex(ValueError, 'incomplete retained'):
                helper.prepare_entra(argparse.Namespace())
            self.assertEqual(private.read_text(), 'retained-private-material')
            self.assertEqual(sorted(path.name for path in Path(directory).iterdir()), ['hub.env'])

    def test_private_preparation_has_no_keycloak_calls_and_preserves_keys_on_rerun(self):
        import argparse
        import os
        with tempfile.TemporaryDirectory() as directory, patch.object(helper, 'STATE', Path(directory)), \
                patch.object(helper.runpy, 'run_path', side_effect=AssertionError('no Keycloak admin')), \
                patch.dict(os.environ, {'VEOVEO_DATABASE_PASSWORD': 'fixture-db-only',
                    'VEOVEO_IDP_OIDC_CLIENT_SECRET': 'fixture-idp-only',
                    'VEOVEO_IDP_OIDC_ISSUER': self.ISSUER}):
            path = Path(directory) / 'discovery.json'
            path.write_text(json.dumps(self.arguments()['discovery']))
            options = argparse.Namespace(hub_origin=self.ORIGIN, oidc_discovery_file=str(path),
                entra_tenant_id=self.TENANT, entra_client_id=self.CLIENT, tenant='reference',
                work_context='reference-work', policy_version='reference-policy', owner_object_id=self.ADMIN,
                member_object_id=[self.MEMBER], admin_object_id=[self.ADMIN], billed_object_id=[self.BILLED],
                application_client_id='opencode', client_redirect_uri=['http://127.0.0.1:3210/callback'],
                native_gateway='https://host.openshell.internal:17670', include_artifact=True, include_media=True, include_embedding=False)
            helper.prepare_entra(options)
            original = {name: (Path(directory) / name).read_bytes()
                        for name in ['hub.env', 'hub.json', 'authorization-server.jwks.json']}
            helper.prepare_entra(options)
            for name, value in original.items():
                self.assertEqual((Path(directory) / name).read_bytes(), value)
                self.assertEqual((Path(directory) / name).stat().st_mode & 0o777, 0o600)
            jwk = json.loads(original['authorization-server.jwks.json'])['keys'][0]
            self.assertEqual(jwk['kid'], json.loads(original['hub.json'])['authorizationServers'][0]['accessTokenKeyId'])
            with patch.dict(os.environ, {'VEOVEO_IDP_OIDC_CLIENT_SECRET': 'different-fixture'}):
                with self.assertRaisesRegex(ValueError, 'credential differs'):
                    helper.prepare_entra(options)
            self.assertEqual((Path(directory) / 'hub.env').read_bytes(), original['hub.env'])
            options.tenant = 'different-tenant'
            with self.assertRaisesRegex(ValueError, 'identity/authority differs'):
                helper.prepare_entra(options)
            self.assertEqual((Path(directory) / 'hub.json').read_bytes(), original['hub.json'])


if __name__ == '__main__':
    unittest.main()
