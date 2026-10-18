#!/usr/bin/env python3
"""PRIVATE three-arm build driver. No execution on import; no runtime/performance claim.
Arm targets are isolated, not proof that every external build input was independently sealed.
"""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import selectors
import shutil
import signal
import stat
import subprocess
import sys
import time

sys.dont_write_bytecode = True

OLD = 'f67c84e48baf96a6b8bc667edca7fb859e852ac5'
CURRENT = '7a5468514967fca9066c2d00025a3ddb83446d39'
TARGET = 'x86_64-unknown-linux-gnu'
LOG_CAP = 64 * 1024 * 1024
MIN_DISK = 20 * 1024**3
MIN_MEMORY = 4 * 1024**3
BUILD_DEADLINE = None
STATUS_PATH = None
STATUS = {'schema_version':1,'arm_code':0,'stage_code':0,'error_code':0}
PACKAGES = ['robe--robin-dls-profile','robe--robin-600x-ledwash','showtec--sunstrip-led-rgb-42206','robe--robin-ledbeam-150','generic--dimmer','venue--stage-element-2-1-m','venue--stage-stairs','venue--four-point-truss','venue--curtain-5-m']
OVERLAYS = {
    'old-legacy': ('old-benchmark-shim.mjs','f67b9eb65a113d33ac8bd49cc16cc26c3cb742bd920e63762d4039c48957129d','apps/light-desktop/src/NativePackagedStageBenchmarkApp.tsx','d348e549f88741561a4f66a7ea4a0b0798a717afeb2d077f87034e6b61a3360b','eb7211d23b11717cdd8a807b998b0868c2545938c4e485b54693928f7f15eed6'),
    'current-legacy': ('apply-startup-compatibility.py','63ad1df173cc691b63a30a490d9e372459299cefacfe546f4c89974b1465f703','crates/light/adapters/headless/src/runtime/e2e_semantic_contract.rs','78f8f1ed1c45ebf88d9c9cf75e39b385ccafbbb1b742348d916c273966c38981','a29077ffc65e4a8f404504a8bed388fac57c8004773239f55169a9fe90409c69'),
}

def digest(data):
    return hashlib.sha256(data).hexdigest()

def regular(path, maximum=None):
    info = path.lstat()
    assert stat.S_ISREG(info.st_mode), 'not an ordinary file'
    if maximum is not None:
        assert info.st_size <= maximum, 'file cap'
    return path.read_bytes()

def save(path, value):
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_TRUNC | os.O_NOFOLLOW, 0o600)
    with os.fdopen(descriptor, 'w') as output:
        json.dump(value, output, sort_keys=True)
        output.write('\n')

def git(root, *arguments):
    remaining = 120 if BUILD_DEADLINE is None else min(120, BUILD_DEADLINE-time.monotonic())
    assert remaining > 0, 'whole-build deadline'
    return subprocess.check_output(['git', *arguments], cwd=root, timeout=remaining)

def tree_manifest(root, revision):
    entries = git(root, 'ls-tree', '-r', '-z', revision).split(b'\0')
    result = {}
    for entry in entries:
        if not entry:
            continue
        metadata, name = entry.split(b'\t', 1)
        assert metadata.split()[0] in (b'100644', b'100755'), 'nonregular Git source'
        name = name.decode()
        result[name] = digest(git(root, 'show', revision + ':' + name))
    return result

def verify(root, original, expected):
    assert set(original) == set(expected)
    for name, wanted in expected.items():
        assert BUILD_DEADLINE is None or time.monotonic() < BUILD_DEADLINE, 'whole-build deadline'
        assert digest(regular(root / name)) == wanted, 'source mismatch'
    status = git(root, 'status', '--porcelain=v1', '-z', '--untracked-files=all').decode().split('\0')
    changed = {entry[3:] for entry in status if entry}
    assert changed == {name for name in original if original[name] != expected[name]}, 'unexpected source mutation'

def arm_environment(root, target, inherited):
    allowed_paths = {'LIGHT_REPOSITORY_ROOT','LIGHT_ARTIFACTS_DIR','LIGHT_TMP_DIR'}
    conflicts = sorted(key for key in inherited if key.startswith('LIGHT_') and
                       (key.endswith('_DIR') or key.endswith('_ROOT')) and key not in allowed_paths)
    assert not conflicts, 'conflicting inherited LIGHT path override'
    assert 'LIGHT_OUTPUT_WORKERS' not in inherited, 'runtime worker override'
    assert 'LIGHT_DESKTOP_SIDECAR_TARGET' not in inherited, 'inherited sidecar binding'
    result = dict(inherited)
    result.update(LIGHT_REPOSITORY_ROOT=str(root), LIGHT_ARTIFACTS_DIR=str(root / '.artifacts'),
                  LIGHT_TMP_DIR=str(root / '.artifacts/tmp'), CARGO_TARGET_DIR=str(target))
    # Build parallelism only; runtime pool/default policies remain untouched.
    result['CARGO_BUILD_JOBS'] = '2'
    return result

def selected_environment(environment):
    names = ['PATH','RUSTUP_TOOLCHAIN','RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','CARGO_PROFILE_RELEASE_OPT_LEVEL',
             'CARGO_PROFILE_RELEASE_LTO','CARGO_PROFILE_RELEASE_DEBUG','CARGO_BUILD_JOBS','NODE_OPTIONS',
             'LIGHT_REPOSITORY_ROOT','LIGHT_ARTIFACTS_DIR','LIGHT_TMP_DIR','CARGO_TARGET_DIR',
             'LIGHT_DESKTOP_SIDECAR_DIR','LIGHT_DESKTOP_SIDECAR_TARGET']
    return {name: environment.get(name) for name in names}

def stop_group(process):
    # Only this command's created process group; no process-name/global searches.
    for kind in [signal.SIGTERM, signal.SIGKILL]:
        try:
            os.killpg(process.pid, kind)
        except ProcessLookupError:
            pass
        try:
            process.wait(timeout=3)
        except subprocess.TimeoutExpired:
            if kind == signal.SIGKILL:
                raise RuntimeError('owned command did not stop')
    end = time.monotonic() + 3
    while True:
        try:
            os.killpg(process.pid, 0)
        except ProcessLookupError:
            return
        assert time.monotonic() < end, 'owned process group survived cleanup'
        time.sleep(0.05)


def execute(arguments, cwd, log, deadline, environment=None, seconds=2100):
    remaining = min(seconds, deadline - time.monotonic())
    assert remaining > 0, 'whole-build deadline'
    descriptor = os.open(log, os.O_WRONLY | os.O_APPEND | os.O_CREAT | os.O_NOFOLLOW, 0o600)
    process = None
    try:
        assert os.fstat(descriptor).st_size <= LOG_CAP, 'log cap'
        process = subprocess.Popen(arguments, cwd=cwd, env=environment, stdout=subprocess.PIPE,
                                   stderr=subprocess.STDOUT, start_new_session=True)
        end = time.monotonic() + remaining
        with selectors.DefaultSelector() as events:
            events.register(process.stdout, selectors.EVENT_READ)
            while events.get_map():
                assert time.monotonic() < end, 'command deadline'
                for key, _ in events.select(min(0.1, max(0, end-time.monotonic()))):
                    chunk = os.read(key.fileobj.fileno(), 65536)
                    if not chunk:
                        events.unregister(key.fileobj)
                        continue
                    assert os.fstat(descriptor).st_size + len(chunk) <= LOG_CAP, 'log cap'
                    with memoryview(chunk) as pending:
                        while pending:
                            count = os.write(descriptor, pending)
                            assert count > 0
                            pending = pending[count:]
            assert process.wait(timeout=max(0.01, end-time.monotonic())) == 0, 'command failed'
    finally:
        try:
            if process is not None:
                try:
                    stop_group(process)
                finally:
                    process.stdout.close()
        finally:
            os.close(descriptor)

def resources(directory):
    free = shutil.disk_usage(directory).free
    assert free >= MIN_DISK, 'less than 20GiB free'
    fields = dict(line.split(':',1) for line in Path('/proc/meminfo').read_text().splitlines())
    available = int(fields['MemAvailable'].split()[0]) * 1024
    assert available >= MIN_MEMORY, 'less than 4GiB available memory'
    return {'free_disk_bytes': free, 'available_memory_bytes': available}

def versions(root, environment, output, deadline, phase):
    result = {}
    for tool in ['rustc','cargo','node','npm']:
        log = output / (tool + '-version-' + phase + '.log')
        execute([tool,'--version'], root, log, deadline, environment, 30)
        result[tool] = regular(log, 65536).decode().strip()
    return result

def binary(path, role):
    info = path.lstat()
    assert stat.S_ISREG(info.st_mode) and 0 < info.st_size <= 2*1024**3, 'binary bound'
    result = hashlib.sha256()
    with path.open('rb') as source:
        assert source.read(4) == b'\x7fELF', 'expected ELF'
        source.seek(0)
        while block := source.read(1024*1024):
            result.update(block)
    assert path.stat().st_size == info.st_size, 'binary changed'
    return {'role':role,'sha256':result.hexdigest(),'bytes':info.st_size}

def packaged(image, output, root, environment, deadline):
    extraction = output / 'packaged-private'
    extraction.mkdir(mode=0o700)
    execute([str(image),'--appimage-extract'], extraction, output/'extract-private.log', deadline, environment, 120)
    appdir = extraction / 'squashfs-root'
    assert appdir.is_dir() and not appdir.is_symlink()
    files, entries = [], 0
    for location, directories, names in os.walk(appdir, followlinks=False):
        entries += len(directories) + len(names)
        assert entries <= 20000, 'packaged entry cap'
        files.extend(Path(location)/name for name in names)
    rows = []
    for name in ['light-headless','viz-renderer','light-desktop']:
        accepted = {name, name+'-'+TARGET} if name != 'light-desktop' else {name}
        candidates = [p for p in files if p.name in accepted and p.is_file() and not p.is_symlink()]
        assert len(candidates) == 1, 'ambiguous packaged ELF'
        assert candidates[0].resolve().is_relative_to(appdir.resolve())
        rows.append(binary(candidates[0], name))
    return rows

def apply_overlay(arm, root, helpers, expected, checkout, log, deadline):
    if arm not in OVERLAYS:
        return None
    helper, wanted, name, before, after = OVERLAYS[arm]
    path = helpers/helper
    assert digest(regular(path, 128*1024)) == wanted, 'overlay helper pin'
    assert expected[name] == before, 'overlay input pin'
    if arm == 'old-legacy':
        execute(['node',str(path),'--source',str(root/name),'--apply'],checkout,log,deadline,seconds=30)
    else:
        execute(['python3','-B',str(path),'--source-root',str(root),'--apply'], checkout, log, deadline, seconds=30)
    assert digest(regular(root/name)) == after, 'overlay output pin'
    expected[name] = after
    return {'helper_sha256':wanted,'path':name,'before_sha256':before,'after_sha256':after}

# Stages: 1 source, 2 overlay, 3 source-check, 4 toolchain, 5 npm,
# 6 frontend, 7 sidecars, 8 config, 9 bundle, 10 extract, 11 final-check,
# 12 owned-target cleanup, 13 arm complete, 14 all complete. Arms 1/2/3.
def mark(arm, stage, error=0):
    STATUS.update(arm_code=arm, stage_code=stage, error_code=error)
    if STATUS_PATH is not None:
        save(STATUS_PATH, STATUS)

def main():
    global BUILD_DEADLINE, STATUS_PATH
    BUILD_DEADLINE = time.monotonic() + 7200
    checkout = Path.cwd().resolve()
    spec = importlib.util.spec_from_file_location('canonical_artifacts', checkout/'tools/artifact_paths.py')
    resolver = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(resolver)
    assert resolver.ROOT == checkout, 'repository root override mismatch'
    temporary = resolver.artifact_path('LIGHT_TMP_DIR','TMP_ROOT').resolve()
    private = Path(sys.argv[1]).resolve()
    assert private.parent == temporary, 'private directory must be direct canonical tmp child'
    temporary.mkdir(parents=True, exist_ok=True)
    private.mkdir(mode=0o700, exist_ok=False)
    STATUS_PATH = private/'build-status-private.json'
    mark(0,0)
    helpers = Path(__file__).resolve().parent
    deadline = BUILD_DEADLINE
    common = {'assets/fixture-library/'+name+'.toskfixture':git(checkout,'show',CURRENT+':assets/fixture-library/'+name+'.toskfixture') for name in PACKAGES}
    receipt = {'schema_version':2,'old_sha':OLD,'current_sha':CURRENT,'helper_sha256':digest(regular(Path(__file__).resolve())),
               'packages':{name:digest(data) for name,data in common.items()},'arms':[],
               'inherited_selected_environment':selected_environment(os.environ),
               'whole_build_input_attested':0,'dependency_freshness_claimed':0,'runtime_or_performance_claimed':0}
    arms = []
    first_versions = None
    for arm_code, (arm, revision) in enumerate([('old-legacy',OLD),('current-legacy',CURRENT),('current-typed',CURRENT)],1):
        mark(arm_code,1)
        assert time.monotonic() < deadline
        initial_resources = resources(private)
        root, output = private/(arm+'-source'), private/arm
        output.mkdir(mode=0o700)
        log, target = output/'build-private.log', output/'cargo-target'
        execute(['git','worktree','add','--detach',str(root),revision],checkout,log,deadline,seconds=120)
        original = tree_manifest(checkout,revision)
        expected = dict(original)
        for name,data in common.items():
            (root/name).write_bytes(data)
            expected[name] = digest(data)
        mark(arm_code,2)
        overlay = apply_overlay(arm,root,helpers,expected,checkout,log,deadline)
        mark(arm_code,3)
        verify(root,original,expected)
        save(output/'source-before.json',expected)
        environment = arm_environment(root,target,os.environ)
        Path(environment['LIGHT_TMP_DIR']).mkdir(parents=True,exist_ok=True)
        before_environment = selected_environment(environment)
        mark(arm_code,4)
        before_versions = versions(root,environment,output,deadline,'before')
        if first_versions is None:
            first_versions = before_versions
        assert before_versions == first_versions, 'cross-arm toolchain changed'
        save(output/'environment-before.json',{'selected':before_environment,'toolchain':before_versions})
        mark(arm_code,5)
        execute(['npm','ci'],root,log,deadline,environment)
        mark(arm_code,6)
        execute(['npm','run','build','--prefix','apps/light-desktop'],root,log,deadline,environment)
        mark(arm_code,7)
        execute(['cargo','build','--release','--locked','--target',TARGET,'-p','light-headless','--bin','light-headless','-p','viz-renderer','--bin','viz-renderer'],root,log,deadline,environment)
        sidecars = output/'sidecars'
        sidecars.mkdir(mode=0o700)
        raw = []
        for name in ['light-headless','viz-renderer']:
            source = target/TARGET/'release'/name
            destination = sidecars/(name+'-'+TARGET)
            shutil.copy2(source,destination)
            raw.append(binary(destination,name))
        environment.update(LIGHT_DESKTOP_SIDECAR_DIR=str(sidecars),LIGHT_DESKTOP_SIDECAR_TARGET=TARGET)
        mark(arm_code,8)
        config = output/'control-config.json'
        execute(['node','tools/write-tauri-artifact-config.mjs','control',str(config)],root,log,deadline,environment)
        mark(arm_code,9)
        execute(['npm','run','--prefix','apps/light-desktop','tauri:build','--','--target',TARGET,'--config',str(config),'--bundles','appimage'],root,log,deadline,environment)
        images = list((target/TARGET/'release/bundle/appimage').glob('*.AppImage'))
        assert len(images) == 1
        image = output/'Control.AppImage'
        shutil.copy2(images[0],image)
        image_receipt = binary(image,'appimage')
        mark(arm_code,10)
        packaged_rows = packaged(image,output,root,environment,deadline)
        assert binary(image,'appimage') == image_receipt, 'AppImage changed during extraction'
        mark(arm_code,11)
        verify(root,original,expected)
        save(output/'source-after.json',expected)
        after_versions = versions(root,environment,output,deadline,'after')
        assert before_versions == after_versions, 'toolchain changed'
        assert selected_environment(environment) == {**before_environment,'LIGHT_DESKTOP_SIDECAR_DIR':str(sidecars),'LIGHT_DESKTOP_SIDECAR_TARGET':TARGET}
        save(output/'environment-after.json',{'selected':selected_environment(environment),'toolchain':after_versions})
        row = {'arm':arm,'source_sha':revision,'source_files':len(original),'effective_manifest_sha256':digest(json.dumps(expected,sort_keys=True).encode()),
               'overlay':overlay,'prebundle_sidecars':raw,'appimage':image_receipt,'packaged_elfs':packaged_rows,
               'isolated_target':1,'resources_before':initial_resources}
        receipt['arms'].append(row)
        save(private/'build-provenance.json',receipt)
        arms.append((root,original,expected))
        mark(arm_code,12)
        assert target.parent == output and target.name == 'cargo-target' and target.is_dir() and not target.is_symlink()
        shutil.rmtree(target)  # Only this arm's generated, owned target; saved receipts/binaries remain.
        mark(arm_code,13)
    mark(3,11)
    for root,original,expected in arms:
        verify(root,original,expected)
    receipt['all_arm_sources_reverified_final'] = 1
    save(private/'build-provenance.json',receipt)
    mark(3,14)

if __name__ == '__main__':
    try:
        main()
    except BaseException as error:
        # Private status only: timeout1/assert2/subprocess3/OS4/other5; no error text.
        code = 1 if isinstance(error, subprocess.TimeoutExpired) else 2 if isinstance(error, AssertionError) else 3 if isinstance(error, subprocess.SubprocessError) else 4 if isinstance(error, OSError) else 5
        mark(STATUS['arm_code'], STATUS['stage_code'], code)
        raise SystemExit(1) from None
