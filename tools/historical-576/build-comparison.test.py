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

    def test_old_secondary_overlay_is_pinned_and_preserves_primary_metadata(self):
        root=self.temporary();helpers=self.temporary()
        first=root/'first';second=root/'timer';first.write_bytes(b'old-first');second.write_bytes(b'old-timer')
        (helpers/'primary.mjs').write_bytes(b'primary');(helpers/'timer.mjs').write_bytes(b'timer')
        primary=('primary.mjs',b.digest(b'primary'),'first',b.digest(b'old-first'),b.digest(b'new-first'))
        timer=('timer.mjs',b.digest(b'timer'),'timer',b.digest(b'old-timer'),b.digest(b'new-timer'))
        expected={'first':primary[3],'timer':timer[3]};calls=[]
        def apply(args,*rest,**kwargs):
            calls.append(args)
            if len(calls)==1:first.write_bytes(b'new-first')
            else:second.write_bytes(b'new-timer')
        with patch.dict(b.OVERLAYS,{'old-legacy':primary}),patch.object(b,'OLD_TIMER_OVERLAY',timer),patch.object(b,'execute',side_effect=apply):
            result=b.apply_overlay('old-legacy',root,helpers,expected,root,root/'log',1)
        self.assertEqual([call[1] for call in calls],[str(helpers/'primary.mjs'),str(helpers/'timer.mjs')])
        self.assertEqual(calls[1],['node',str(helpers/'timer.mjs'),'--source',str(second),'--apply'])
        self.assertEqual(result,{'helper_sha256':primary[1],'path':'first','before_sha256':primary[3],'after_sha256':primary[4],'additional_overlays':[{'helper_sha256':timer[1],'path':'timer','before_sha256':timer[3],'after_sha256':timer[4]}]})
        self.assertEqual(expected,{'first':primary[4],'timer':timer[4]})

    def test_secondary_overlay_refuses_helper_input_and_output_hash_forgery(self):
        for mode in ['helper','manifest','input','output']:
            with self.subTest(mode=mode):
                root=self.temporary();helpers=self.temporary();(root/'first').write_bytes(b'first');(root/'timer').write_bytes(b'timer')
                (helpers/'primary.mjs').write_bytes(b'primary');(helpers/'timer.mjs').write_bytes(b'helper')
                primary=('primary.mjs',b.digest(b'primary'),'first',b.digest(b'first'),b.digest(b'first'))
                timer=('timer.mjs',b.digest(b'helper'),'timer',b.digest(b'timer'),b.digest(b'new'))
                expected={'first':primary[3],'timer':timer[3]}
                if mode=='helper':(helpers/'timer.mjs').write_bytes(b'forged')
                if mode=='manifest':expected['timer']=b.digest(b'forged')
                if mode=='input':(root/'timer').write_bytes(b'forged')
                with patch.dict(b.OVERLAYS,{'old-legacy':primary}),patch.object(b,'OLD_TIMER_OVERLAY',timer),patch.object(b,'execute') as execute,self.assertRaises(AssertionError):
                    b.apply_overlay('old-legacy',root,helpers,expected,root,root/'log',1)
                self.assertEqual(execute.call_count,2 if mode=='output' else 1)

    def test_secondary_timer_scope_and_frozen_current_arms_unchanged(self):
        self.assertEqual(b.OLD_TIMER_OVERLAY,('old-runtime-timer-type.mjs','448e15b5db1bff8269dd99675c25ee5bb22846ad3cadcff989e91c5d75aacb3e','apps/light-desktop/src/api/client/runtime.ts','18db82fddd820e898995ee1ff962d27c9d43810ca7dda376cdaf882548df82f9','8dfeb552e904e1f1af78a2c4e6ec29a954f52fc3d6d768bb3d9fa33ed47a973f'))
        with patch.object(b,'execute') as execute:
            self.assertIsNone(b.apply_overlay('current-typed',Path('/unused'),Path('/unused'),{},Path('/unused'),Path('/unused'),1))
        execute.assert_not_called()
        self.assertEqual(b.OVERLAYS['old-legacy'][1],'f67b9eb65a113d33ac8bd49cc16cc26c3cb742bd920e63762d4039c48957129d')

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

    def tauri_fixture(self):
        root=self.temporary();(root/'application.rs').write_bytes(b'unchanged source')
        original={'application.rs':b.digest(b'unchanged source')}
        for i,name in enumerate(b.TAURI_OUTPUT_FILES[:3]):
            file=root/name;file.parent.mkdir(parents=True,exist_ok=True);file.write_text('{"before":'+str(i)+'}')
            original[name]=b.digest(file.read_bytes())
        return root,original,dict(original)

    def generated_fixture(self):
        root,original,expected=self.tauri_fixture()
        for i,name in enumerate(b.TAURI_OUTPUT_FILES):(root/name).write_text('{"generated":'+str(i)+'}')
        generated=b.tauri_outputs(root)
        status=b''.join(((' M ' if name in original else '?? ')+name+'\0').encode() for name in b.TAURI_OUTPUT_FILES)
        return root,original,expected,generated,status

    def test_tauri_outputs_are_separate_from_strict_input_manifest(self):
        root,original,expected=self.tauri_fixture()
        with patch.object(b,'git',return_value=b''):b.verify(root,original,expected)
        # Prebuild verification still rejects even the later permitted Linux output.
        (root/b.TAURI_OUTPUT_FILES[3]).write_text('{}')
        with patch.object(b,'git',return_value=('?? '+b.TAURI_OUTPUT_FILES[3]+'\0').encode()),self.assertRaises(AssertionError):b.verify(root,original,expected)
        root,original,expected,generated,status=self.generated_fixture()
        with patch.object(b,'git',return_value=status):observed=b.verify_postbuild(root,original,expected,generated)
        self.assertEqual(len(observed),len(original)+1)
        self.assertEqual(set(observed)-set(original),{b.TAURI_OUTPUT_FILES[3]})
        self.assertEqual(expected,original)
        self.assertNotEqual(observed[b.TAURI_OUTPUT_FILES[0]],expected[b.TAURI_OUTPUT_FILES[0]])
        self.assertEqual(b.compact_hash({'z':'last','a':'first'}),b.digest(b'{"a":"first","z":"last"}'))

    def test_later_tauri_drift_or_any_other_source_mutation_is_rejected(self):
        for mode in ['editor','source','untracked','missing inventory']:
            with self.subTest(mode=mode):
                root,original,expected,generated,status=self.generated_fixture()
                if mode=='editor':(root/b.TAURI_OUTPUT_FILES[0]).write_text('{"later":true}')
                if mode=='source':(root/'application.rs').write_bytes(b'changed')
                if mode=='untracked':status+=b'?? unexpected.rs\0'
                if mode=='missing inventory':generated.pop(b.TAURI_OUTPUT_FILES[0])
                with patch.object(b,'git',return_value=status),self.assertRaises(AssertionError):b.verify_postbuild(root,original,expected,generated)

    def test_exact_tauri_jsons_are_regular_bounded_objects(self):
        for mode in ['array','invalid','nan','oversize','symlink','missing','parent symlink']:
            with self.subTest(mode=mode):
                root,original,expected,generated,status=self.generated_fixture();file=root/b.TAURI_OUTPUT_FILES[0]
                if mode=='array':file.write_text('[]')
                if mode=='invalid':file.write_text('{')
                if mode=='nan':file.write_text('{"x":NaN}')
                if mode=='oversize':file.write_bytes(b' '* (b.TAURI_OUTPUT_LIMIT+1))
                if mode=='symlink':file.unlink();file.symlink_to(root/b.TAURI_OUTPUT_FILES[1])
                if mode=='missing':file.unlink()
                if mode=='parent symlink':
                    directory=file.parent;held=directory.with_name('held');directory.rename(held);directory.symlink_to(held,target_is_directory=True)
                with self.assertRaises((AssertionError,OSError,ValueError)):b.tauri_outputs(root)

if __name__=='__main__':unittest.main()
