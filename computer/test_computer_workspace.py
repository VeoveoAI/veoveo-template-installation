"""Isolated shell-helper fixtures; native Lore text matches its CLI output contract."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SOURCE = Path(__file__).with_name('computer-workspace')
REPOSITORY = '1' * 32
BRANCH = '2' * 32
OTHER_BRANCH = '3' * 32
REMOTE = 'grpcs://source.example:41337/' + REPOSITORY

# No native authentication, service or repository is used. The mv fixture supplies
# GNU -T --no-clobber semantics on Mac as well as Linux; the shell runs unchanged.
NATIVE = r'''
import json
import os
from pathlib import Path
import sys

args = sys.argv[1:]
config = json.loads(Path(os.environ['FIXTURE_CONFIG']).read_text())
with open(os.environ['FIXTURE_CALLS'], 'a') as log:
    log.write(json.dumps([Path(sys.argv[0]).name, *args]) + '\n')
if Path(sys.argv[0]).name == 'mv':
    assert args[:2] == ['-T', '--no-clobber']
    if not Path(args[3]).exists():
        os.rename(args[2], args[3])
    sys.exit(0)

checkout = None
if '--repository' in args:
    i = args.index('--repository')
    checkout = Path(args[i + 1])
    del args[i:i + 2]
args = [arg for arg in args if arg not in ('-P', '--local')]
if args[:1] == ['clone']:
    if config['empty'] and '--branch' in args:
        sys.exit(88)
    selected = args[args.index('--branch') + 1] if '--branch' in args else config['default']
    state = {
        'repository': config.get('cloned_repository', config['repository']),
        'branch': config.get('cloned_branch', selected),
        'remote': config.get('cloned_remote', config['remote']),
    }
    target = Path(args[2]) / '.lore'
    target.mkdir(parents=True)
    (target / 'fixture.json').write_text(json.dumps(state))
    sys.exit(0)
if checkout is not None:
    state = json.loads((checkout / '.lore/fixture.json').read_text())
    if args == ['repository', 'config', 'get', 'remote_url']:
        print(state['remote'])
    elif args == ['branch', 'info']:
        print('Branch main\n  ID: ' + state['branch'] + '\n  Latest: ' + '0' * 64)
    elif args == ['repository', 'info']:
        # Only the remote admission read can fail after local identity checks.
        if '--local' not in sys.argv and config.get('auth_failure'):
            sys.exit(config['auth_failure'])
        print('Workspace (' + state['repository'] + ')')
    else:
        raise AssertionError(args)
else:
    assert args == ['repository', 'info', config['remote'] + '/' + config['repository']], args
    print('Workspace (' + config['repository'] + ')\n\nDefault branch: main (' + config['default'] + ')')
'''


class WorkspaceCheckoutTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='computer-workspace-fixture-')
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.sandbox = self.root / 'sandbox'
        self.destination = self.sandbox / 'projects/workspace'
        self.config = {
            'repository': REPOSITORY,
            'default': BRANCH,
            'remote': REMOTE.rsplit('/', 1)[0],
            'empty': False,
        }
        self.config_path = self.root / 'fixture.json'
        self.calls_path = self.root / 'calls.jsonl'
        self.bin = self.root / 'bin'
        self.bin.mkdir()
        for name in ('lore', 'mv'):
            program = self.bin / name
            program.write_text('#!' + sys.executable + '\n' + NATIVE)
            program.chmod(0o755)
        descriptor = self.sandbox / 'home/.config/computer/workspace.env'
        descriptor.parent.mkdir(parents=True)
        descriptor.write_text(
            f"COMPUTER_WORKSPACE_REMOTE='{REMOTE}'\n"
            f"COMPUTER_WORKSPACE_REPOSITORY='{REPOSITORY}'\n"
            f"COMPUTER_WORKSPACE_BRANCH='{BRANCH}'\n"
        )
        self.helper = self.root / 'computer-workspace'
        self.helper.write_text(SOURCE.read_text().replace(
            '/usr/local/bin/lore', str(self.bin / 'lore')
        ).replace('/sandbox', str(self.sandbox)))

    def run_helper(self):
        self.config_path.write_text(json.dumps(self.config))
        return subprocess.run(
            ['/bin/sh', str(self.helper), '--retry'],
            env={**os.environ, 'PATH': str(self.bin) + os.pathsep + os.environ['PATH'],
                 'FIXTURE_CONFIG': str(self.config_path), 'FIXTURE_CALLS': str(self.calls_path)},
            capture_output=True, text=True, timeout=10,
        )

    def calls(self):
        return [json.loads(line) for line in self.calls_path.read_text().splitlines()]

    def existing(self, **changes):
        native = self.destination / '.lore'
        native.mkdir(parents=True)
        state = {'repository': REPOSITORY, 'branch': BRANCH, 'remote': self.config['remote']}
        state.update(changes)
        (native / 'fixture.json').write_text(json.dumps(state))
        (self.destination / 'uncommitted.txt').write_text('leave unchanged')

    def test_empty_default_uses_native_ordinary_clone(self):
        self.config['empty'] = True
        result = self.run_helper()
        self.assertEqual(result.returncode, 0, result.stderr)
        clone = next(call for call in self.calls() if call[1] == 'clone')
        self.assertNotIn('--branch', clone)
        self.assertTrue((self.destination / '.lore').is_dir())
        self.assertFalse(list((self.sandbox / 'projects').glob('.computer-workspace.*')))

    def test_changed_default_selects_retained_branch_explicitly(self):
        self.config['default'] = OTHER_BRANCH
        result = self.run_helper()
        self.assertEqual(result.returncode, 0, result.stderr)
        clone = next(call for call in self.calls() if call[1] == 'clone')
        self.assertEqual(clone[-2:], ['--branch', BRANCH])

    def test_staged_identity_mismatches_never_publish(self):
        for change in ({'cloned_branch': OTHER_BRANCH},
                       {'cloned_repository': '4' * 32},
                       {'cloned_remote': 'grpcs://other.example:41337'}):
            with self.subTest(change=change):
                self.config.update(change)
                result = self.run_helper()
                self.assertEqual(result.returncode, 73, result.stderr)
                self.assertFalse(self.destination.exists())
                self.assertFalse(any(call[0] == 'mv' for call in self.calls()))
                self.assertFalse(list((self.sandbox / 'projects').glob('.computer-workspace.*')))
                for key in change:
                    del self.config[key]

    def test_existing_valid_checkout_is_verified_without_clone(self):
        self.existing()
        result = self.run_helper()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(any(call[1] == 'clone' for call in self.calls()))
        self.assertEqual((self.destination / 'uncommitted.txt').read_text(), 'leave unchanged')

    def test_existing_wrong_branch_is_left_unchanged(self):
        self.existing(branch=OTHER_BRANCH)
        result = self.run_helper()
        self.assertEqual(result.returncode, 73, result.stderr)
        self.assertFalse(any(call[1] == 'clone' for call in self.calls()))
        self.assertEqual((self.destination / 'uncommitted.txt').read_text(), 'leave unchanged')

    def test_native_auth_refusal_is_preserved_before_publication(self):
        self.config['auth_failure'] = 17
        result = self.run_helper()
        self.assertEqual(result.returncode, 17, result.stderr)
        self.assertFalse(self.destination.exists())
        self.assertFalse(any(call[0] == 'mv' for call in self.calls()))


if __name__ == '__main__':
    unittest.main()
