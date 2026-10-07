#!/usr/bin/env python3
"""Unix install/uninstall lifecycle on a synthetic home and fake scheduler.
No actual job is registered, no cache GC is run, no user's files are used.
"""
from pathlib import Path
import tempfile, subprocess, os, platform, shutil, plistlib
root=Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='rldc-installer-') as directory:
    fixture=Path(directory).resolve(); home=fixture/'home & unicode тест';home.mkdir()
    stage=fixture/'package';stage.mkdir();stub=fixture/'commands';stub.mkdir()
    shutil.copy2(root/'target/debug/rldyour-cleaner',stage/'rldyour-cleaner')
    for file in ['install.sh','uninstall.sh']:shutil.copy2(root/file,stage/file)
    shutil.copytree(root/'platforms',stage/'platforms')
    log=fixture/'scheduler.log'
    for name in ['systemctl','launchctl']:
        file=stub/name;file.write_text('#!/bin/sh\nprintf "%s\\n" "$*" >> "$RLD_TEST_LOG"\n');file.chmod(0o755)
    env=dict(os.environ,HOME=str(home),XDG_CONFIG_HOME=str(home/'custom-config'),RLD_TEST_LOG=str(log),PATH=str(stub)+os.pathsep+os.environ['PATH'])
    # No binary from PATH is used for cache discovery: installers only validate config.
    subprocess.run(['bash',str(stage/'install.sh')],env=env,check=True)
    config=(home/'Library/Application Support/rldyour-cleaner/config.toml') if platform.system()=='Darwin' else home/'custom-config/rldyour-cleaner/config.toml'
    assert config.is_file() and (home/'.local/bin/rldyour-cleaner').is_file()
    original=config.read_bytes()
    if platform.system()=='Darwin':
        with (home/'Library/LaunchAgents/io.nddev.rldyour-cleaner.plist').open('rb') as stream:agent=plistlib.load(stream)
        assert agent['ProgramArguments'][0]==str(home/'.local/bin/rldyour-cleaner')
        assert agent['StartCalendarInterval']=={'Hour':3,'Minute':0}
    else:
        assert (home/'custom-config/systemd/user/rldyour-cleaner.timer').is_file()
    assert 'kickstart' not in log.read_text() and 'start rldyour-cleaner.service' not in log.read_text()
    subprocess.run(['bash',str(stage/'install.sh')],env=env,check=True);assert config.read_bytes()==original
    subprocess.run(['bash',str(stage/'uninstall.sh')],env=env,check=True)
    assert not (home/'.local/bin/rldyour-cleaner').exists() and config.read_bytes()==original
    print('PASS: platform installer, custom home/XDG paths, upgrade preserves policy, uninstall preserves state; scheduler stub only')
