"""Local, secret-free regression checks; never contact Keycloak or Lore."""
import contextlib
import copy
import importlib.machinery
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import tomllib
import unittest
from unittest.mock import patch
import uuid

SOURCE = Path(__file__).with_name('configure-lore')
loader = importlib.machinery.SourceFileLoader('configure_lore', str(SOURCE))
spec = importlib.util.spec_from_loader(loader.name, loader)
helper = importlib.util.module_from_spec(spec)
loader.exec_module(helper)
REPOSITORY = '1' * 32
BRANCH = '2' * 32
FOREIGN = str(uuid.UUID(int=9))


class Issuer:
    def __init__(self):
        self.scopes = [{'name': 'basic', 'id': 'basic'}, {'name': 'roles', 'id': 'roles'}]
        self.clients = {}
        self.links = {}
        self.mappers = {}

    def request(self, path, method='GET', data=None, token=None, form=False):
        if path == '/realms/master/protocol/openid-connect/token':
            return {'access_token': 'fixture-admin-token'}
        path = path.removeprefix('/admin/realms/veoveo/')
        if path == 'client-scopes':
            if method == 'POST':
                self.scopes.append({**data, 'id': data['name']})
            return copy.deepcopy(self.scopes)
        if path.startswith('client-scopes/'):
            key = path.split('/')[1]
            entries = self.mappers.setdefault(key, [])
            if method == 'POST':
                entries.append({**data, 'id': data['name']})
            elif method == 'PUT':
                entries[:] = [{**data, 'id': item['id']} if item['id'] == path.split('/')[-1] else item for item in entries]
            return copy.deepcopy(entries)
        if path.startswith('clients?clientId='):
            return copy.deepcopy([client for client in self.clients.values() if client['clientId'] == path.split('=', 1)[1]])
        if path == 'clients' and method == 'POST':
            key = data['clientId']
            self.clients[key] = {**data, 'id': key}
            return None
        parts = path.split('/')
        if parts[0] != 'clients' or len(parts) < 2:
            raise AssertionError('unexpected issuer route')
        key = parts[1]
        client = self.clients[key]
        if len(parts) == 2:
            if method == 'PUT':
                self.clients[key] = {**data, 'id': key}
            return copy.deepcopy(self.clients[key])
        if parts[2] == 'client-secret':
            return {'value': client['secret']}
        if parts[2] == 'service-account-user':
            return {'id': str(uuid.uuid5(uuid.NAMESPACE_URL, key))}
        if parts[2] in {'default-client-scopes', 'optional-client-scopes'}:
            links = self.links.setdefault((key, parts[2]), {'roles'})
            if method == 'DELETE':
                links.discard(parts[3])
            elif method == 'PUT':
                links.add(parts[3])
            return [{'id': item} for item in links]
        # In particular, no role mappings may be changed.
        raise AssertionError('unexpected issuer route')


class ConfigureLoreTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        for path in ['identity', 'state/identity', 'lore', 'openshell/providers']:
            (self.root / path).mkdir(parents=True, exist_ok=True)
        (self.root / 'state/identity/credentials.json').write_text(json.dumps({'adminUsername': 'test', 'adminPassword': 'fixture-only'}))
        (self.root / 'lore/server.toml').write_text('[hooks.factory_branch]\nenabled = true\nassignments = {}\n')
        (self.root / 'openshell/providers/lore-time.example.yaml').write_text('scope: lore-repo-REPOSITORY_ID\n')
        self.issuer = Issuer()
        self.patch_root = patch.object(helper, 'ROOT', self.root)
        self.patch_root.start()
        self.addCleanup(self.patch_root.stop)

    def invoke(self, *args):
        with patch('sys.argv', ['configure-lore', *args]), patch.object(helper.runpy, 'run_path', return_value={'request': self.issuer.request}), contextlib.redirect_stdout(io.StringIO()):
            helper.main()

    def named(self, *extra):
        self.invoke('--client-id', 'vv-lore-time', '--repository-id', REPOSITORY, *extra)

    def state(self):
        return json.loads((self.root / 'state/openshell/lore-direct/config.json').read_text())

    def assignments(self):
        return tomllib.loads((self.root / 'state/openshell/lore-direct/server.toml').read_text())['hooks']['factory_branch']['assignments']

    def test_named_mode_needs_no_fixture_and_links_only_exact_optional_repository(self):
        self.named()
        self.assertEqual(set(self.issuer.clients), {'vv-lore-time'})
        self.assertEqual(self.issuer.links[('vv-lore-time', 'default-client-scopes')], {'basic'})
        self.assertEqual(self.issuer.links[('vv-lore-time', 'optional-client-scopes')], {'lore-repo-' + REPOSITORY})
        self.assertEqual(self.assignments(), {})
        grant = self.issuer.mappers['lore-repo-' + REPOSITORY]
        resources = next(item for item in grant if item['name'] == 'lore-repository')
        self.assertEqual(json.loads(resources['config']['claim.value']), [{'resource_id': 'urc-' + REPOSITORY, 'permission': []}])
        for path in (self.root / 'state/openshell/lore-direct').iterdir():
            self.assertEqual(path.stat().st_mode & 0o777, 0o600)
        self.assertEqual((path.parent.stat().st_mode & 0o777), 0o700)

    def test_named_mode_ignores_malformed_existing_fixture(self):
        fixture_path = self.root / 'state/openshell/factory-hook-fixture.json'
        fixture_path.parent.mkdir(parents=True, exist_ok=True)
        fixture_path.write_text('not valid JSON')
        self.named('--assigned-branch-id', BRANCH)
        saved = self.state()['clients']['time']
        self.named()
        self.assertEqual(self.state()['clients']['time'], saved)
        self.assertEqual(self.assignments()[saved['subject']]['branch'], BRANCH)
        self.assertEqual(fixture_path.read_text(), 'not valid JSON')

    def test_smoke_repository_is_reserved_even_without_fixture_file(self):
        self.assertFalse((self.root / 'state/openshell/factory-hook-fixture.json').exists())
        with self.assertRaises(ValueError):
            self.invoke('--client-id', 'vv-lore-time', '--repository-id',
                        '01a13b791bca7fe182896b5cbc1b56a5')
        self.assertEqual(self.issuer.clients, {})

    def test_rerun_preserves_disabled_client_secret_subject_roles_and_final_assignment(self):
        self.named('--assigned-branch-id', BRANCH)
        saved = self.state()['clients']['time']
        client = self.issuer.clients['vv-lore-time']
        client['enabled'] = False
        client['realmRoles'] = ['preserved-operator-role']
        client['attributes']['operator-note'] = 'retain'
        old_secret = client['secret']
        self.named()  # Initial-phase command cannot erase finalized assignment.
        self.assertEqual(self.state()['clients']['time'], saved)
        self.assertEqual(self.assignments()[saved['subject']]['branch'], BRANCH)
        current = self.issuer.clients['vv-lore-time']
        self.assertFalse(current['enabled'])
        self.assertEqual(current['secret'], old_secret)
        self.assertEqual(current['realmRoles'], ['preserved-operator-role'])
        self.assertEqual(current['attributes']['operator-note'], 'retain')

    def test_named_mode_preserves_fixture_config_and_unrelated_trusted_assignment(self):
        directory = self.root / 'state/openshell/lore-direct'
        directory.mkdir(parents=True)
        fixture = {'clientId': 'vv-lore-a', 'repository': '3' * 32, 'branch': '4' * 32, 'subject': FOREIGN}
        (directory / 'config.json').write_text(json.dumps({'issuer': helper.ISSUER, 'audience': helper.AUDIENCE, 'clients': {'a': fixture}, 'fixture': {'preserve': True}}))
        server = '[hooks.factory_branch]\nenabled = true\n\n[hooks.factory_branch.assignments."' + FOREIGN + '"]\nrepository = "' + '3' * 32 + '"\nbranch = "' + '5' * 32 + '"\n'
        (directory / 'server.toml').write_text(server)
        self.named('--assigned-branch-id', BRANCH)
        self.assertEqual(self.state()['clients']['a'], fixture)
        self.assertEqual(self.state()['fixture'], {'preserve': True})
        self.assertTrue((directory / 'server.toml').read_text().startswith(server))
        self.assertEqual(self.assignments()[FOREIGN]['branch'], '5' * 32)

    def test_fixture_rerun_keeps_time_client_and_assignment(self):
        self.named('--assigned-branch-id', BRANCH)
        time_client = self.state()['clients']['time']
        fixture = {'repository': '3' * 32, 'assigned': '4' * 32, 'main': '5' * 32,
                   'other': '6' * 32, 'forbidden': '7' * 32}
        (self.root / 'state/openshell/factory-hook-fixture.json').write_text(json.dumps(fixture))
        self.invoke()
        self.assertEqual(self.state()['clients']['time'], time_client)
        self.assertEqual(self.assignments()[time_client['subject']]['branch'], BRANCH)
        self.assertEqual(self.state()['fixture'], fixture)
        before = self.assignments()
        self.invoke()
        self.assertEqual(self.assignments(), before)

    def test_adding_assignment_does_not_remove_other_empty_inline_tables(self):
        path = self.root / 'lore/server.toml'
        path.write_text(path.read_text() + '\n[other]\nassignments = {}\n')
        self.named('--assigned-branch-id', BRANCH)
        server = tomllib.loads((self.root / 'state/openshell/lore-direct/server.toml').read_text())
        self.assertEqual(server['other'], {'assignments': {}})

    def test_secret_mismatch_fails_without_rotating(self):
        self.named()
        self.issuer.clients['vv-lore-time']['secret'] = 'changed-by-operator'
        with self.assertRaises(ValueError):
            self.named()
        self.assertEqual(self.issuer.clients['vv-lore-time']['secret'], 'changed-by-operator')

    def test_reserved_fixture_client_cannot_be_adopted(self):
        with self.assertRaises(ValueError):
            self.invoke('--client-id', 'vv-lore-a', '--repository-id', REPOSITORY)
        self.assertEqual(self.issuer.clients, {})

    def test_explicit_branch_change_retains_unrelated_tables_and_is_idempotent(self):
        self.named('--assigned-branch-id', BRANCH)
        directory = self.root / 'state/openshell/lore-direct'
        path = directory / 'server.toml'
        path.write_text(path.read_text() + '\n[server.auth]\njwt_typ = "at+jwt"\n')
        self.named('--assigned-branch-id', '6' * 32)
        before = path.read_text()
        self.named('--assigned-branch-id', '6' * 32)
        self.assertEqual(path.read_text(), before)
        parsed = tomllib.loads(before)
        self.assertEqual(parsed['server']['auth']['jwt_typ'], 'at+jwt')
        self.assertEqual(next(iter(self.assignments().values()))['branch'], '6' * 32)


if __name__ == '__main__':
    unittest.main()
