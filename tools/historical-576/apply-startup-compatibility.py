"""PRIVATE CI-only current compatibility arm overlay; never a shipping environment switch.
Default attests only. --apply changes exactly one startup return in an explicitly supplied tree.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import stat

SOURCE_HEAD='7a5468514967fca9066c2d00025a3ddb83446d39'
SOURCE_SHA='78f8f1ed1c45ebf88d9c9cf75e39b385ccafbbb1b742348d916c273966c38981'
RELATIVE='crates/light/adapters/headless/src/runtime/e2e_semantic_contract.rs'
OLD=b"""pub(super) fn startup_programming_contract() -> u16 {
    #[cfg(test)]
    if let Some(contract) = startup_contract_override::current() {
        return contract;
    }
    supported_programming_contract()
}"""
NEW=OLD.replace(b'    supported_programming_contract()\n}',b'    0\n}')


def sha(value):return hashlib.sha256(value).hexdigest()


def transform(value):
    if sha(value)!=SOURCE_SHA:raise ValueError('unattested source')
    if value.count(OLD)!=1 or value.count(b'fn startup_programming_contract(')!=1:raise ValueError('ambiguous startup function')
    transformed=value.replace(OLD,NEW,1)
    if transformed.replace(NEW,OLD,1)!=value:raise ValueError('nonminimal overlay')
    return transformed


def overlay(source_root,apply=False):
    source_root=Path(source_root)
    if not source_root.is_dir() or source_root.is_symlink():raise ValueError('invalid source root')
    source=source_root/RELATIVE
    for parent in [source,*source.parents]:
        if parent==source_root.parent:break
        if parent.is_symlink():raise ValueError('symlink source')
    descriptor=os.open(source,(os.O_RDWR if apply else os.O_RDONLY)|os.O_NOFOLLOW)
    try:
        info=os.fstat(descriptor)
        if not stat.S_ISREG(info.st_mode) or not 0<info.st_size<=128*1024:raise ValueError('invalid source file')
        original=b''
        while len(original)<info.st_size:
            block=os.read(descriptor,info.st_size-len(original))
            if not block:raise ValueError('short source')
            original+=block
        transformed=transform(original)
        if apply:
            os.lseek(descriptor,0,os.SEEK_SET);pending=transformed
            while pending:
                written=os.write(descriptor,pending)
                if written<=0:raise ValueError('short overlay write')
                pending=pending[written:]
            os.ftruncate(descriptor,len(transformed));os.fsync(descriptor)
            os.lseek(descriptor,0,os.SEEK_SET)
            if os.read(descriptor,len(transformed)+1)!=transformed:raise ValueError('overlay verification failed')
        return dict(schema_version=1,current_compatibility_arm=1,source_file_revision=SOURCE_HEAD,source_sha256=sha(original),overlay_sha256=sha(transformed),overlay_applied=int(apply),startup_contract=0,production_supported_constant_changed=0,production_compile_time_assertions_changed=0,engine_source_files_changed=0,dynamics_source_files_changed=0,persisted_schema_changed=0,shipping_environment_switch_added=0,raw_repository_writer_contract_changed=0,performance_claim=0)
    finally:os.close(descriptor)


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--source-root',required=True);parser.add_argument('--apply',action='store_true');args=parser.parse_args()
    try:print(json.dumps(overlay(args.source_root,args.apply),sort_keys=True))
    except (OSError,ValueError):print('{"overlay_rejected":1,"source_attested":0}');raise SystemExit(2)


if __name__=='__main__':main()
