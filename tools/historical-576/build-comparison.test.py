"""Pure mocks/private fixtures only: never calls driver main, compiler or AppImage."""
import ast
import hashlib
import itertools
import importlib.util
import os
from pathlib import Path
import signal
import tempfile
import unittest
from unittest.mock import Mock, patch
import sys
sys.dont_write_bytecode = True
HERE=Path(__file__).resolve().parent
ROOT=next(parent for parent in HERE.parents if (parent/'tools/artifact_paths.py').is_file())
artifact_spec=importlib.util.spec_from_file_location('test_artifacts',ROOT/'tools/artifact_paths.py')
artifacts=importlib.util.module_from_spec(artifact_spec);artifact_spec.loader.exec_module(artifacts)
TEMPORARY=artifacts.artifact_path('LIGHT_TMP_DIR','TMP_ROOT')
TEMPORARY.mkdir(parents=True,exist_ok=True)
spec=importlib.util.spec_from_file_location('builder',HERE/'build-comparison.py')
b=importlib.util.module_from_spec(spec);spec.loader.exec_module(b)

class Pure(unittest.TestCase):
    def temporary(self):
        temporary=tempfile.TemporaryDirectory(dir=TEMPORARY,prefix='pure-')
        self.addCleanup(temporary.cleanup)
        return Path(temporary.name)

    def test_python310_syntax_and_import_has_no_execution(self):
        ast.parse((HERE/'build-comparison.py').read_text(),feature_version=(3,10))
        self.assertEqual(b.CURRENT,'7a5468514967fca9066c2d00025a3ddb83446d39')

    def test_arm_paths_isolate_targets_and_reject_conflicting_overrides(self):
        root=Path('/owned/old-source');target=Path('/owned/old-output/cargo-target')
        env=b.arm_environment(root,target,{'PATH':'/usr/bin','LIGHT_TMP_DIR':'/canonical/tmp','CARGO_BUILD_JOBS':'999'})
        self.assertEqual(env['CARGO_TARGET_DIR'],str(target))
        self.assertEqual(env['LIGHT_REPOSITORY_ROOT'],str(root))
        self.assertEqual(env['LIGHT_TMP_DIR'],str(root/'.artifacts/tmp'))
        self.assertNotIn('LIGHT_OUTPUT_WORKERS',env)
        self.assertEqual(env['CARGO_BUILD_JOBS'],'2')
        for conflict in ['LIGHT_CONTROL_FRONTEND_DIR','LIGHT_OUTPUT_WORKERS','LIGHT_DESKTOP_SIDECAR_TARGET']:
            with self.subTest(conflict=conflict),self.assertRaises(AssertionError):
                b.arm_environment(root,target,{conflict:'unexpected'})

    def test_verifier_rejects_source_drift_and_untracked_file(self):
        root=self.temporary();(root/'a').write_bytes(b'one');original={'a':b.digest(b'one')}
        with patch.object(b,'git',return_value=b''):
            b.verify(root,original,original)
            (root/'a').write_bytes(b'two')
            with self.assertRaises(AssertionError):b.verify(root,original,original)
        (root/'a').write_bytes(b'one')
        with patch.object(b,'git',return_value=b'?? unknown\0'),self.assertRaises(AssertionError):
            b.verify(root,original,original)

    def test_source_manifest_refuses_symlink_git_entries(self):
        with patch.object(b,'git',return_value=b'120000 blob abcd\tlink\0'),self.assertRaises(AssertionError):
            b.tree_manifest(Path('/unused'),'revision')

    def test_overlay_requires_helper_input_output_pins_and_exact_cli(self):
        root=self.temporary();helpers=self.temporary();source=root/'source';source.write_bytes(b'old')
        helper=helpers/'apply-startup-compatibility.py';helper.write_bytes(b'helper')
        entry=(helper.name,b.digest(b'helper'),'source',b.digest(b'old'),b.digest(b'new'))
        expected={'source':b.digest(b'old')}
        def apply(args,*rest,**kwargs):
            self.assertEqual(args,['python3','-B',str(helper),'--source-root',str(root),'--apply'])
            source.write_bytes(b'new')
        with patch.dict(b.OVERLAYS,{'current-legacy':entry}),patch.object(b,'execute',side_effect=apply):
            b.apply_overlay('current-legacy',root,helpers,expected,root,root/'log',1)
        self.assertEqual(expected['source'],b.digest(b'new'))
        helper.write_bytes(b'changed')
        with patch.dict(b.OVERLAYS,{'current-legacy':entry}),patch.object(b,'execute') as execute,self.assertRaises(AssertionError):
            b.apply_overlay('current-legacy',root,helpers,expected,root,root/'log',1)
        execute.assert_not_called()

    def test_owned_cleanup_uses_only_created_group_and_reaps(self):
        process=Mock(pid=12345)
        def signal_owned(pid,kind):
            if kind==0:raise ProcessLookupError
        with patch.object(os,'killpg',side_effect=signal_owned) as kill:
            b.stop_group(process)
        self.assertEqual(kill.call_args_list[0].args,(12345,signal.SIGTERM))
        self.assertEqual(kill.call_args_list[1].args,(12345,signal.SIGKILL))
        self.assertEqual(process.wait.call_count,2)

    def test_surviving_descendant_group_fails_before_next_phase(self):
        process=Mock(pid=12345);clock=itertools.count()
        with patch.object(os,'killpg') as kill,patch.object(b.time,'monotonic',side_effect=lambda:next(clock)),patch.object(b.time,'sleep'),self.assertRaises(AssertionError):
            b.stop_group(process)
        self.assertEqual(process.wait.call_count,2)
        self.assertTrue(any(call.args==(12345,0) for call in kill.call_args_list))

    def test_numeric_status_is_closed_and_has_no_error_text(self):
        old_path=b.STATUS_PATH;old_status=dict(b.STATUS)
        self.addCleanup(setattr,b,'STATUS_PATH',old_path)
        self.addCleanup(b.STATUS.update,old_status)
        b.STATUS_PATH=Path('/never-written')
        with patch.object(b,'save') as save:
            b.mark(2,9,2)
        self.assertEqual(save.call_args.args[1],{'schema_version':1,'arm_code':2,'stage_code':9,'error_code':2})

    def test_deadline_and_logcap_fail_before_command_spawn(self):
        root=self.temporary();log=root/'private.log'
        with patch.object(b.subprocess,'Popen') as spawn,self.assertRaises(AssertionError):
            b.execute(['never'],root,log,0)
        spawn.assert_not_called()
        log.write_bytes(b'oversize')
        with patch.object(b,'LOG_CAP',1),patch.object(b.subprocess,'Popen') as spawn,self.assertRaises(AssertionError):
            b.execute(['never'],root,log,b.time.monotonic()+1)
        spawn.assert_not_called()

    def test_binary_and_private_receipt_are_bounded_and_distinct(self):
        root=self.temporary();elf=root/'elf';elf.write_bytes(b'\x7fELFpayload')
        row=b.binary(elf,'packaged')
        self.assertEqual(row,{'role':'packaged','sha256':hashlib.sha256(elf.read_bytes()).hexdigest(),'bytes':11})
        link=root/'link';link.symlink_to(elf)
        with self.assertRaises(AssertionError):b.binary(link,'bad')
        b.save(root/'receipt.json',row)
        self.assertEqual((root/'receipt.json').stat().st_mode&0o777,0o600)

    def test_packaged_inventory_requires_all_three_exact_elfs(self):
        root=self.temporary();output=root/'out';output.mkdir();env={}
        def extract(*args,**kwargs):
            bundle=output/'packaged-private/squashfs-root/usr/bin';bundle.mkdir(parents=True)
            for name in ['light-headless','viz-renderer','light-desktop']:(bundle/name).write_bytes(b'\x7fELF'+name.encode())
        with patch.object(b,'execute',side_effect=extract):
            rows=b.packaged(root/'never-executed.AppImage',output,root,env,1)
        self.assertEqual([r['role'] for r in rows],['light-headless','viz-renderer','light-desktop'])

if __name__=='__main__':unittest.main()
