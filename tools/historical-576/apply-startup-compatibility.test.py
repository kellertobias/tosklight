"""Pure source-overlay tests; no production checkout, build, server, or network."""
import hashlib
import importlib.util
from pathlib import Path
import tempfile
import subprocess
import unittest
from unittest import mock

spec=importlib.util.spec_from_file_location('overlay',Path(__file__).with_name('apply-startup-compatibility.py'));p=importlib.util.module_from_spec(spec);spec.loader.exec_module(p)
DIRECTORY=Path(__file__).resolve().parent
ROOT=Path.cwd().resolve()
artifact_spec=importlib.util.spec_from_file_location('artifact_paths',ROOT/'tools/artifact_paths.py')
artifacts=importlib.util.module_from_spec(artifact_spec);artifact_spec.loader.exec_module(artifacts)
TEMP_DIRECTORY=artifacts.artifact_path('LIGHT_TMP_DIR','TMP_ROOT')
TEMP_DIRECTORY.mkdir(parents=True,exist_ok=True)
FROZEN=subprocess.check_output(['git','show','7a5468514967fca9066c2d00025a3ddb83446d39:'+str(p.RELATIVE)],cwd=ROOT)
assert hashlib.sha256(FROZEN).hexdigest()==p.SOURCE_SHA


class Pure(unittest.TestCase):
    def test_frozen_source_is_exact_and_change_is_only_the_startup_return(self):
        self.assertEqual(hashlib.sha256(FROZEN).hexdigest(),p.SOURCE_SHA)
        after=p.transform(FROZEN);self.assertNotEqual(after,FROZEN)
        self.assertEqual(after.replace(p.NEW,p.OLD,1),FROZEN)
        self.assertEqual(after.count(p.NEW),1);self.assertEqual(after.count(p.OLD),0)
        prefix,tail=FROZEN.split(p.OLD);self.assertEqual(after,prefix+p.NEW+tail)
        self.assertEqual(p.NEW,p.OLD.replace(b'    supported_programming_contract()\n}',b'    0\n}'))
        self.assertIn(b'PRODUCTION_PROGRAMMING_CONTRACT: u16 = SUPPORTED_PROGRAMMING_CONTRACT;',after)
        self.assertIn(b'assert!(supported_programming_contract() == SUPPORTED_PROGRAMMING_CONTRACT);',after)
        self.assertNotIn(b'std::env',after)

    def test_unattested_modified_or_already_overlaid_source_is_refused(self):
        for invalid in [b'',FROZEN+b'\n',FROZEN.replace(b'PRODUCTION_PROGRAMMING_CONTRACT',b'OTHER',1),p.transform(FROZEN)]:
            with self.assertRaises(ValueError):p.transform(invalid)

    def test_missing_or_duplicate_occurrence_is_refused_even_under_a_hash_fixture(self):
        for value in [FROZEN.replace(p.OLD,b''),FROZEN+p.OLD,FROZEN+b'fn startup_programming_contract(']:
            with mock.patch.object(p,'SOURCE_SHA',hashlib.sha256(value).hexdigest()):
                with self.assertRaises(ValueError):p.transform(value)

    def test_default_is_read_only_and_explicit_apply_preserves_file_mode(self):
        with tempfile.TemporaryDirectory(dir=TEMP_DIRECTORY) as temporary:
            root=Path(temporary);file=root/p.RELATIVE;file.parent.mkdir(parents=True);file.write_bytes(FROZEN);file.chmod(0o640)
            report=p.overlay(root);self.assertEqual(file.read_bytes(),FROZEN);self.assertEqual(report['overlay_applied'],0)
            result=p.overlay(root,True);self.assertEqual(file.read_bytes(),p.transform(FROZEN));self.assertEqual(file.stat().st_mode&0o777,0o640);self.assertEqual(result['overlay_applied'],1);self.assertEqual(result['current_compatibility_arm'],1)
            for key in ['production_supported_constant_changed','production_compile_time_assertions_changed','engine_source_files_changed','dynamics_source_files_changed','persisted_schema_changed','shipping_environment_switch_added','raw_repository_writer_contract_changed','performance_claim']:self.assertEqual(result[key],0)
            with self.assertRaises(ValueError):p.overlay(root,True)

    def test_mismatch_and_symlinks_never_modify_source_or_destination(self):
        with tempfile.TemporaryDirectory(dir=TEMP_DIRECTORY) as temporary:
            root=Path(temporary);file=root/p.RELATIVE;file.parent.mkdir(parents=True);file.write_bytes(b'WRONG')
            with self.assertRaises(ValueError):p.overlay(root,True)
            self.assertEqual(file.read_bytes(),b'WRONG')
            original=root/'original';original.write_bytes(FROZEN);file.unlink();file.symlink_to(original)
            with self.assertRaises(ValueError):p.overlay(root,True)
            self.assertEqual(original.read_bytes(),FROZEN)
            link=root/'source-link';link.symlink_to(root)
            with self.assertRaises(ValueError):p.overlay(link,True)

    def test_no_source_root_default_or_production_environment_hook(self):
        body=Path(p.__file__).read_text();self.assertIn("add_argument('--source-root',required=True)",body);self.assertIn("add_argument('--apply',action='store_true')",body);self.assertNotIn('os.environ',body)

if __name__=='__main__':unittest.main()
