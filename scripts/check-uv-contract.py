#!/usr/bin/env python3
"""Real vendor uv contract checks; only private synthetic caches are mutated.
No package downloads, user cache/profile changes or actual project data.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile
import shutil

parser = argparse.ArgumentParser()
parser.add_argument('--uv', required=True)
parser.add_argument('--cleaner', required=True)
args = parser.parse_args()
uv = str(Path(shutil.which(args.uv) or args.uv).resolve())
cleaner_path = Path(args.cleaner)
if os.name == 'nt' and not cleaner_path.exists():
    cleaner_path = cleaner_path.with_suffix('.exe')
cleaner = str(cleaner_path.resolve())
env = dict(os.environ)
for key in ['UV_NO_CACHE', 'UV_PROJECT', 'UV_WORKING_DIR', 'UV_CONFIG_FILE', 'UV_LINK_MODE']:
    env.pop(key, None)

def run(command, *, check=True):
    return subprocess.run(command, env=env, capture_output=True, text=True,
                          timeout=10, check=check)

with tempfile.TemporaryDirectory(prefix='rldc-real-uv-') as temporary:
    root = Path(temporary).resolve()
    env.update(HOME=str(root), USERPROFILE=str(root), LOCALAPPDATA=str(root/'local'),
               XDG_STATE_HOME=str(root/'state'), XDG_CACHE_HOME=str(root/'xdg-cache'))
    cache, victim = root/'cache', root/'victim'
    env['UV_CACHE_DIR'] = str(cache)
    out = run([uv, 'cache', 'dir', '--offline', '--no-config', '--cache-dir', str(cache)])
    assert Path(out.stdout.strip()).resolve() == cache
    assert not cache.exists(), 'cache directory discovery must be read-only'
    cached = cache/'environments-v2'/'linked-project'
    cached.mkdir(parents=True)
    (cached/'needed').write_text('synthetic-environment-content')
    policy = root/'policy.toml'
    policy.write_text('[native_gc]\nuv = true\n')
    out = run([cleaner, '--config', str(policy), 'run', '--dry-run', '--json'])
    assert json.loads(out.stdout)['caches'][0]['action'] == 'kept'
    assert (cached/'needed').exists(), 'legacy enable cannot authorize env removal'
    victim.mkdir()
    (victim/'needed').write_text('synthetic-external-content')
    if os.name != 'nt':
        (cache/'environments-v2'/'external-link').symlink_to(victim, target_is_directory=True)
        (root/'project-venv').symlink_to(cached, target_is_directory=True)
        import fcntl
        with (cache/'.lock').open('a+b') as lease:
            fcntl.flock(lease, fcntl.LOCK_SH)
            env['UV_LOCK_TIMEOUT'] = '1'
            blocked = run([uv, 'cache', 'prune', '--offline', '--no-config',
                           '--cache-dir', str(cache)], check=False)
            assert blocked.returncode != 0, 'native GC must honor an in-use cache lease'
            assert (cached/'needed').exists()
    env['UV_LOCK_TIMEOUT'] = '1'
    # Demonstrate why default uv prune was disabled: this intentionally removes
    # the synthetic cached environment, including a project's linked target.
    run([uv, 'cache', 'prune', '--offline', '--no-config', '--cache-dir', str(cache)])
    assert not cached.exists(), 'native prune removes cached environments, not just dangling entries'
    assert (victim/'needed').exists(), 'symlink target outside cache must survive'
    if os.name != 'nt':
        assert (root/'project-venv').is_symlink() and not (root/'project-venv').exists()
print('PASS: real uv read-only discovery, default preservation and cached-env removal; POSIX also checks native lease and external-link safety')
