#!/usr/bin/env python3
"""Transfer generated documentation captures without committing PNG output."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import zipfile

ROOT = Path(__file__).resolve().parents[1]


def declared_files() -> set[str]:
    paths: set[str] = set()
    for gallery in ('help', 'marketing'):
        manifest = json.loads((ROOT / f'docs/{gallery}/screenshot-manifest.json').read_text())
        for entry in manifest['entries']:
            paths.add(f'docs/{gallery}/assets/screenshots/{entry["file"]}')
            if entry.get('sourceFile'):
                paths.add(entry['sourceFile'])
    for name in paths:
        if not name.startswith(('docs/help/assets/screenshots/', 'docs/marketing/assets/screenshots/', 'docs/marketing/assets/stills/')) or '..' in Path(name).parts:
            raise ValueError(f'Capture outside permitted asset directories: {name}')
    return paths


def transfer(mode: str, archive: Path, commit: str) -> None:
    paths = declared_files()
    manifests = {f'docs/{gallery}/screenshot-manifest.json': hashlib.sha256((ROOT / f'docs/{gallery}/screenshot-manifest.json').read_bytes()).hexdigest() for gallery in ('help', 'marketing')}
    if mode == 'pack':
        files = {}
        for name in sorted(paths):
            content = (ROOT / name).read_bytes()
            if not content.startswith(b'\x89PNG\r\n\x1a\n'):
                raise ValueError(f'Not a PNG capture: {name}')
            files[name] = {'sha256': hashlib.sha256(content).hexdigest(), 'bytes': len(content)}
        metadata = {'sourceCommit': commit, 'manifestSHA256': manifests, 'files': files}
        archive.parent.mkdir(parents=True, exist_ok=True)
        with zipfile.ZipFile(archive, 'w', compression=zipfile.ZIP_DEFLATED) as bundle:
            bundle.writestr('captures.json', json.dumps(metadata, indent=2) + '\n')
            for name in sorted(paths):
                bundle.write(ROOT / name, name)
    else:
        with zipfile.ZipFile(archive) as bundle:
            if sum(info.file_size for info in bundle.infolist()) > 150_000_000:
                raise ValueError('Capture archive exceeds 150 MB')
            metadata = json.loads(bundle.read('captures.json'))
            names = bundle.namelist()
            if len(names) != len(set(names)) or set(names) != paths | {'captures.json'}:
                raise ValueError('Archive must contain exactly the current declared captures and metadata')
            if metadata['sourceCommit'] != commit or metadata.get('manifestSHA256') != manifests or set(metadata['files']) != paths:
                raise ValueError('Capture source commit or manifest differs from the checked-out manual')
            # Validate the entire archive before replacing any working files. Never extract paths.
            for name in sorted(paths):
                content = bundle.read(name)
                expected = metadata['files'][name]
                if not content.startswith(b'\x89PNG\r\n\x1a\n') or len(content) != expected['bytes'] or hashlib.sha256(content).hexdigest() != expected['sha256']:
                    raise ValueError(f'Invalid capture or checksum: {name}')
            for name in sorted(paths):
                target = ROOT / name
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(bundle.read(name))
    print(f'{mode}: {len(paths)} declared captures for {commit}; archive SHA256 {hashlib.sha256(archive.read_bytes()).hexdigest()}')


def restore_native(archive: Path) -> None:
    """Restore reviewed native-only source photographs; regenerate all browser captures."""
    allowed = declared_files()
    sources = set()
    for gallery in ('help', 'marketing'):
        manifest = json.loads((ROOT / f'docs/{gallery}/screenshot-manifest.json').read_text())
        sources.update(entry['sourceFile'] for entry in manifest['entries'] if entry.get('sourceFile'))
    if not sources or not sources.issubset(allowed):
        raise ValueError('No valid native capture sources declared')
    with zipfile.ZipFile(archive) as bundle:
        if sum(info.file_size for info in bundle.infolist()) > 150_000_000:
            raise ValueError('Capture archive exceeds 150 MB')
        names = bundle.namelist()
        if len(names) != len(set(names)):
            raise ValueError('Duplicate capture archive entries')
        metadata = json.loads(bundle.read('captures.json'))
        for name in sorted(sources):
            if name not in metadata['files']:
                raise ValueError(f'Reviewed native source missing: {name}')
            content = bundle.read(name)
            expected = metadata['files'][name]
            if not content.startswith(b'\x89PNG\r\n\x1a\n') or len(content) != expected['bytes'] or hashlib.sha256(content).hexdigest() != expected['sha256']:
                raise ValueError(f'Invalid native capture or checksum: {name}')
        for name in sorted(sources):
            target = ROOT / name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(bundle.read(name))
    print(f'Restored {len(sources)} reviewed native inputs from capture source {metadata["sourceCommit"]}; browser captures must be regenerated for current HEAD')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=('pack', 'restore', 'restore-native'))
    parser.add_argument('archive', type=Path)
    args = parser.parse_args()
    sha = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    if args.mode == 'restore-native':
        restore_native(args.archive)
    else:
        transfer(args.mode, args.archive, sha)
