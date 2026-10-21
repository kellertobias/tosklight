"""Publication must reject mismatched, tampered and path-injecting capture archives."""
import importlib.util
import json
from pathlib import Path
import tempfile
import sys
sys.dont_write_bytecode = True
import unittest
import zipfile

spec = importlib.util.spec_from_file_location('manual_captures', Path(__file__).with_name('manual-captures.py'))
captures = importlib.util.module_from_spec(spec)
spec.loader.exec_module(captures)


class CaptureTransferTests(unittest.TestCase):
    def setUp(self):
        scratch = Path(__file__).resolve().parents[1] / '.artifacts/tmp'
        scratch.mkdir(parents=True, exist_ok=True)
        self.temp = tempfile.TemporaryDirectory(prefix='manual-captures-test-', dir=scratch)
        self.addCleanup(self.temp.cleanup)
        captures.ROOT = Path(self.temp.name)
        self.name = 'docs/help/assets/screenshots/example.png'
        for gallery in ('help', 'marketing'):
            manifest = captures.ROOT / f'docs/{gallery}/screenshot-manifest.json'
            manifest.parent.mkdir(parents=True)
            manifest.write_text(json.dumps({'entries': [{'file': 'example.png'}] if gallery == 'help' else []}))
        target = captures.ROOT / self.name
        target.parent.mkdir(parents=True)
        self.content = b'\x89PNG\r\n\x1a\nexample'
        target.write_bytes(self.content)
        self.archive = captures.ROOT / 'captures.zip'
        captures.transfer('pack', self.archive, 'a' * 40)

    def test_restores_exact_verified_capture(self):
        target = captures.ROOT / self.name
        target.write_bytes(b'stale')
        captures.transfer('restore', self.archive, 'a' * 40)
        self.assertEqual(target.read_bytes(), self.content)

    def test_rejects_another_source_revision_before_mutating(self):
        target = captures.ROOT / self.name
        target.write_bytes(b'preserve')
        with self.assertRaisesRegex(ValueError, 'source commit'):
            captures.transfer('restore', self.archive, 'b' * 40)
        self.assertEqual(target.read_bytes(), b'preserve')

    def test_rejects_tampered_capture_before_mutating(self):
        with zipfile.ZipFile(self.archive) as bundle:
            metadata = bundle.read('captures.json')
        with zipfile.ZipFile(self.archive, 'w') as bundle:
            bundle.writestr('captures.json', metadata)
            bundle.writestr(self.name, b'\x89PNG\r\n\x1a\ntampered')
        target = captures.ROOT / self.name
        target.write_bytes(b'preserve')
        with self.assertRaisesRegex(ValueError, 'checksum'):
            captures.transfer('restore', self.archive, 'a' * 40)
        self.assertEqual(target.read_bytes(), b'preserve')

    def test_rejects_changed_manifest_even_when_paths_are_equal(self):
        manifest = captures.ROOT / 'docs/help/screenshot-manifest.json'
        data = json.loads(manifest.read_text())
        data['entries'][0]['title'] = 'Changed screenshot intent'
        manifest.write_text(json.dumps(data))
        with self.assertRaisesRegex(ValueError, 'manifest differs'):
            captures.transfer('restore', self.archive, 'a' * 40)

    def test_rejects_extra_or_traversal_entry(self):
        with zipfile.ZipFile(self.archive, 'a') as bundle:
            bundle.writestr('../outside.png', b'unexpected')
        with self.assertRaisesRegex(ValueError, 'exactly'):
            captures.transfer('restore', self.archive, 'a' * 40)


if __name__ == '__main__':
    unittest.main()
