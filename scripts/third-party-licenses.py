#!/usr/bin/env python3
"""Produce redistribution notices from locked crate sources, without build-time HTTP."""
import json
import subprocess
from pathlib import Path
ROOT = Path(__file__).resolve().parent.parent
metadata = json.loads(subprocess.check_output(['cargo','metadata','--locked','--format-version','1'], cwd=ROOT))
# List the entire workspace dependency set. Some packages are build/optional/other-target only.
sections = ['''Third-party Rust dependency license notices
==========================================
Generated from Cargo.lock. This is a conservative workspace-wide inventory, including
build, development and platform-specific dependencies. Inclusion does not imply that
every listed package is linked into each artifact. OR expressions offer alternatives;
MIT or Apache-2.0 is selected where available. UniFFI components use MPL-2.0; their
unmodified source is available at the exact crates.io package URLs below, and the
license text is provided. Original clins-rs files retain their declared Apache/MIT terms.
''']
missing = []
for package in sorted(metadata['packages'], key=lambda p:(p['name'],p['version'])):
    if package['id'] in metadata['workspace_members']:
        continue
    name, version = package['name'], package['version']
    directory = Path(package['manifest_path']).parent
    files = sorted(f for f in directory.iterdir() if f.is_file() and any(f.name.upper().startswith(prefix) for prefix in ['LICENSE','LICENCE','COPYING','COPYRIGHT','NOTICE']))
    if package.get('license_file'):
        files = sorted(set(files + [directory / package['license_file']]))
    if not files:
        fallback = 'uniffi' if name.startswith('uniffi') else name
        supplemental = ROOT / 'LICENSES/supplemental' / f'{fallback}.txt'
        if supplemental.exists():
            files = [supplemental]
        elif name == 'r-efi':
            # The UEFI-only crate publishes its grant and author list in README/AUTHORS.
            files = [directory / 'README.md']
            if (directory/'AUTHORS').exists(): files.append(directory/'AUTHORS')
        else:
            missing.append(f'{name}@{version}')
            continue
    sections.append(f'\n{name} {version}\nLicense expression: {package["license"]}\nSource: https://crates.io/api/v1/crates/{name}/{version}/download\n')
    for file in files:
        sections.append(f'\n--- {file.name} ---\n{file.read_text()}\n')
if missing:
    raise SystemExit('Missing license files: ' + ', '.join(missing))
output = '\n'.join(sections)
(ROOT/'THIRD_PARTY_LICENSES.txt').write_text(output)
# Distributions contain the notice alongside their compiled Rust library.
for binding in ['python','typescript','kotlin']:
    (ROOT/'bindings'/binding/'THIRD_PARTY_LICENSES.txt').write_text(output)
print(f'Wrote redistribution notices for {len(metadata["packages"])-len(metadata["workspace_members"])} locked dependency packages')
