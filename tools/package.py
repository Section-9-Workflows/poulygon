#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Bundle the native i386 executable without changing the target system libraries."""
import json
from pathlib import Path
import re
import shutil
import subprocess

root = Path('dist')
libraries = root / 'lib'
libraries.mkdir(exist_ok=True)
listing = subprocess.check_output(['ldd', str(root / 'bin/facet')], text=True)
if 'not found' in listing:
    raise SystemExit(listing)
packages = {}
for line in listing.splitlines():
    found = re.search(r'(?:=>\s+)?(/\S+)\s+\(', line)
    if not found:
        continue
    path = Path(found[1])
    shutil.copy2(path, libraries / path.name)
    candidates = [str(path), str(path.resolve())]
    candidates += [name[4:] for name in candidates if name.startswith('/usr/')]
    owner = None
    for name in dict.fromkeys(candidates):
        result = subprocess.run(['dpkg-query', '-S', name], capture_output=True, text=True)
        if result.returncode == 0:
            owner = result.stdout.split(': ')[0]
            break
    if not owner:
        raise SystemExit('Runtime library has no package owner: ' + str(path))
    packages[owner] = subprocess.check_output(
        ['dpkg-query', '-W', '-f=${Version}\t${source:Package}\t${source:Version}', owner], text=True)
    notice = root / 'licenses' / owner.replace(':', '_')
    notice.mkdir(parents=True, exist_ok=True)
    shutil.copy2(Path('/usr/share/doc') / owner.split(':')[0] / 'copyright', notice / 'copyright')
if not (libraries / 'ld-linux.so.2').exists():
    raise SystemExit('Expected the i386 loader')
shutil.copytree('/usr/share/common-licenses', root / 'licenses/common', dirs_exist_ok=True)
shutil.copytree('vendor/poulsback/LICENSES', root / 'licenses/poulsback', dirs_exist_ok=True)
shutil.copy2('vendor/poulsback/LICENSE', root / 'licenses/poulsback/LICENSE')
shutil.copy2('vendor/poulsback/NOTICES.md', root / 'licenses/poulsback/NOTICES.md')
shutil.copy2('vendor/poulsback/REVISION', root / 'poulsback-revision.txt')
(root / 'runtime-packages.json').write_text(json.dumps(packages, indent=2) + '\n')
for name in ['README.md', 'LICENSE']:
    shutil.copy2(name, root / name)
shutil.copy2('tools/facet', root / 'facet')
(root / 'facet').chmod(0o755)
