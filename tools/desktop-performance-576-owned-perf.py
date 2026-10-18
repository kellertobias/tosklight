"""Explicit opt-in, one-owned-headless Linux user-CPU stack capture.

Importing/testing never executes tools. Raw files stay in a newly created private spool.
Synthetic DWARF capability is NOT application unwind proof: an actual 2s canonical
Rust capture must first have a resolved owned Rust chain of depth >=2. No codegen,
workload, priorities, kernel settings, runtime symbols or acceptance gates are changed.
Fixed categories describe symbol spelling in the verified owned ELF, not causal CPU
stages; other DSOs remain unverified. Inclusive rows count each category once/sample.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import resource
import shutil
import signal
import stat
import struct
import subprocess
import sys
import time
import threading

sys.dont_write_bytecode = True
from artifact_paths import ROOT, artifact_path

CAP = 64 * 1024 * 1024
TASK_CAP, SAMPLE_CAP, FRAME_CAP, LINE_CAP = 256, 10000, 128, 4096
# 0 unresolved; 1 sampling; 2 preparation; 3 composition; 4 other dynamics;
# 5 engine; 6 headless; 7 std/core/alloc; 8 other owned; 9 unverified absolute DSO;
# 10 vdso; 11 other nonabsolute DSO; 12 owned Rayon; 13 owned allocation/drop. No class implies a caller or exclusive stage.
CATEGORIES = tuple(range(14))
REQUEST_KEYS = {'schema', 'candidate_sha', 'runner_pid', 'runner_start_ticks',
                'root_pid', 'root_start_ticks', 'target_pid', 'target_start_ticks',
                'uid', 'elf_sha256', 'elf_build_id', 'duration_seconds', 'allow_root', 'perf_path', 'perf_sha256'}
FAILURES = {name: i + 1 for i, name in enumerate((
    'request', 'ownership', 'identity', 'threads', 'maps', 'elf', 'tool',
    'deadline', 'bytes', 'record', 'census', 'decode', 'unwind', 'cleanup', 'private', 'source', 'loss'))}


# Fixed first-failed census predicates only. No observed header values are retained.
# 1 magic/size; 2 file layout; 3 attr identity/size; 4 unsupported sample bits;
# 5 missing sample bits; 6 scope flags; 7 clock; 8 record header bounds;
# 9 record length; 10 record cap; 11 sample mode; 12 time-conv size;
# 13 time-conv shift; 14 time-conv capabilities; 15 time-conv reserved;
# 16 lost length; 17 unknown record; 18 sample cap.
# Linux v5.15 include/uapi/linux/perf_event.h sample-format bit positions.
# Details 19..32 identify the lowest known unsupported bit; 4 remains the unknown
# fallback. These fixed names never expose an observed mask or header value.
# READ, STREAM_ID, RAW, BRANCH_STACK, WEIGHT, DATA_SRC, TRANSACTION, REGS_INTR,
# PHYS_ADDR, AUX, CGROUP, DATA_PAGE_SIZE, CODE_PAGE_SIZE, WEIGHT_STRUCT.
UNSUPPORTED_SAMPLE_BITS = (4, 9, 10, 11, 14, 15, 17, 18, 19, 20, 21, 22, 23, 24)
CENSUS_DETAIL_MAX = 32


def unsupported_sample_detail(bits):
    return next((19 + index for index, bit in enumerate(UNSUPPORTED_SAMPLE_BITS)
                 if bits & (1 << bit)), 4)


class Refused(RuntimeError):
    def __init__(self, code, detail=0):
        assert type(detail) is int and 0 <= detail <= CENSUS_DETAIL_MAX
        assert code == 'census' or detail == 0
        self.code = FAILURES[code]
        self.detail = detail
        super().__init__(code)


def require(value, code, detail=0):
    if not value:
        raise Refused(code, detail)


def uint(value):
    return type(value) is int and 0 <= value <= 2**53 - 1


def digest(data):
    return hashlib.sha256(data).hexdigest()


def bounded_read(path, cap=CAP):
    with open(path, 'rb') as stream:
        value = stream.read(cap + 1)
    require(len(value) <= cap, 'bytes')
    return value


def identity(pid, proc=Path('/proc')):
    require(type(pid) is int and 0 < pid <= 2**31 - 1, 'identity')
    root = proc / str(pid)
    text = bounded_read(root / 'stat', 8192).decode()
    prefix, fields = text.rsplit(')', 1)
    require(prefix.startswith(str(pid) + ' (') and len(fields.split()) >= 20, 'identity')
    fields = fields.split()
    status = dict(line.split(':', 1) for line in bounded_read(root / 'status', 16384).decode().splitlines() if ':' in line)
    return {'pid': pid, 'ppid': int(fields[1]), 'start': int(fields[19]),
            'tgid': int(status['Tgid']), 'uid': int(status['Uid'].split()[0])}


def file_identity(info):
    return (info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns, info.st_ctime_ns)


def validate_request(request):
    require(type(request) is dict and set(request) == REQUEST_KEYS and type(request['schema']) is int and request['schema'] == 1, 'request')
    for key in ('runner_pid', 'root_pid', 'target_pid'):
        require(uint(request[key]) and 0 < request[key] <= 2**31 - 1, 'request')
    for key in ('runner_start_ticks', 'root_start_ticks', 'target_start_ticks', 'uid'):
        require(uint(request[key]), 'request')
    require(0 < request['uid'] <= 2**31 - 1 and request['allow_root'] in (0, 1)
            and type(request['allow_root']) is int, 'request')
    require(type(request['duration_seconds']) is int and 1 <= request['duration_seconds'] <= 10, 'request')
    for key, pattern in (('candidate_sha', '[0-9a-f]{40}'), ('elf_sha256', '[0-9a-f]{64}'),
                         ('elf_build_id', '[0-9a-f]{8,128}'), ('perf_sha256', '[0-9a-f]{64}')):
        require(type(request[key]) is str and re.fullmatch(pattern, request[key]), 'request')
    require(type(request['perf_path']) is str and len(request['perf_path']) <= 4096
            and Path(request['perf_path']).is_absolute(), 'request')
    require(os.getuid() == request['uid'] or (os.getuid() == 0 and request['allow_root'] == 1), 'ownership')
    return request


def ancestry(pid, ancestor, proc=Path('/proc')):
    seen = set()
    for _ in range(128):
        if pid == ancestor:
            return True
        if pid <= 1 or pid in seen:
            return False
        seen.add(pid)
        pid = identity(pid, proc)['ppid']
    raise Refused('ownership')


def verify_owner(request):
    values = {}
    for role in ('runner', 'root', 'target'):
        value = identity(request[role + '_pid'])
        require(value['start'] == request[role + '_start_ticks'] and value['uid'] == request['uid']
                and value['tgid'] == value['pid'], 'ownership')
        values[role] = value
    require(ancestry(os.getpid(), values['runner']['pid']), 'ownership')
    require(values['root']['ppid'] == values['runner']['pid']
            and ancestry(values['target']['pid'], values['root']['pid']), 'ownership')
    return values


def threads(pid, uid):
    begin = time.monotonic()
    entries = list((Path('/proc') / str(pid) / 'task').iterdir())
    require(0 < len(entries) <= TASK_CAP, 'threads')
    result = {}
    for entry in entries:
        value = identity(int(entry.name))
        require(value['uid'] == uid and value['tgid'] == pid, 'threads')
        result[value['pid']] = value
        require(time.monotonic() - begin <= 2, 'deadline')
    require(set(result) == {int(p.name) for p in (Path('/proc') / str(pid) / 'task').iterdir()}, 'threads')
    return result


def maps(pid, executable_stat):
    raw = bounded_read(Path('/proc') / str(pid) / 'maps', 1024 * 1024)
    rows = []
    for line in raw.decode().splitlines():
        require(len(line) <= LINE_CAP, 'maps')
        fields = line.split(maxsplit=5)
        require(len(fields) >= 5 and re.fullmatch('[r-][w-][x-][ps]', fields[1]), 'maps')
        bounds = fields[0].split('-'); device = fields[3].split(':')
        require(len(bounds) == len(device) == 2, 'maps')
        start, end, offset = int(bounds[0], 16), int(bounds[1], 16), int(fields[2], 16)
        require(0 <= start < end < 2**64 and 0 <= offset < 2**64, 'maps')
        if int(fields[4]) == executable_stat.st_ino and os.makedev(*(int(n, 16) for n in device)) == executable_stat.st_dev:
            if 'x' in fields[1]:
                rows.append((start, end, offset))
    require(rows, 'maps')
    return raw, rows


def environment():
    return {'LC_ALL': 'C', 'LANG': 'C',
            'PERF_CONFIG': '/dev/null', 'PERF_CONFIG_NOSYSTEM': '1', 'PERF_CONFIG_NOGLOBAL': '1',
            'DEBUGINFOD_URLS': '', 'PERF_BUILDID_DIR': '/nonexistent'}


def stop_owned(child, pidfd):
    previous = signal.pthread_sigmask(signal.SIG_BLOCK, {signal.SIGINT, signal.SIGTERM})
    try:
        try:
            return _stop_owned(child, pidfd)
        except (OSError, subprocess.SubprocessError):
            raise Refused('cleanup') from None
    finally:
        signal.pthread_sigmask(signal.SIG_SETMASK, previous)


def _stop_owned(child, pidfd):
    for sig, wait in ((signal.SIGINT, 2), (signal.SIGTERM, 1), (signal.SIGKILL, 1)):
        if child.poll() is not None:
            return child.returncode
        try:
            if pidfd is not None:
                signal.pidfd_send_signal(pidfd, sig)
            else:
                child.send_signal(sig)  # Unreaped Popen child cannot have a recycled PID.
        except ProcessLookupError:
            pass
        try:
            return child.wait(timeout=wait)
        except subprocess.TimeoutExpired:
            pass
    raise Refused('cleanup')


def private_file(path):
    return os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)


def child_limits():
    resource.setrlimit(resource.RLIMIT_FSIZE, (CAP, CAP))
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))


def run_tool(argv, spool, name, deadline, seconds=None, guard=None, window=None, extra_env=None):
    """Bound owned tool output and lifetime; recorder target is never signaled here."""
    require(extra_env is None or (set(extra_env) == {'TMPDIR'} and Path(extra_env['TMPDIR']).is_absolute()), 'private')
    child_env = environment(); child_env.update(extra_env or {})
    output, errors = spool / (name + '.stdout'), spool / (name + '.stderr')
    child = None; pidfd = None; timer = None
    elapsed = threading.Event()
    with os.fdopen(private_file(output), 'wb') as out, os.fdopen(private_file(errors), 'wb') as err:
        try:
            child = subprocess.Popen(argv, cwd=spool, env=child_env, stdout=out, stderr=err,
                                     stdin=subprocess.DEVNULL, close_fds=True, umask=0o077, preexec_fn=child_limits)
            pidfd = os.pidfd_open(child.pid)
            if seconds is not None:
                if window is not None:
                    window['begin'] = time.monotonic()
                def expire():
                    elapsed.set()
                    try:
                        signal.pidfd_send_signal(pidfd, signal.SIGINT)
                    except ProcessLookupError:
                        pass
                timer = threading.Timer(seconds, expire)
                timer.start()
            next_check = 0
            while child.poll() is None:
                now = time.monotonic()
                require(now < deadline, 'deadline')
                for path in (output, errors, spool / 'perf.data'):
                    if path.exists():
                        require(path.stat().st_size <= CAP, 'bytes')
                if guard and now >= next_check:
                    guard(); next_check = now + .25
                if elapsed.is_set():
                    stop_owned(child, pidfd); break
                time.sleep(.02)
            if seconds is not None:
                require(elapsed.is_set(), 'record')  # Early recorder exit is never a successful interval.
            for path in (output, errors, spool / 'perf.data'):
                if path.exists():
                    require(path.stat().st_size < CAP, 'bytes')
            require(child.returncode in (0, -signal.SIGINT) if seconds is not None else child.returncode == 0, 'tool')
            require(time.monotonic() <= deadline, 'deadline')
            if guard:
                guard()
            return bounded_read(output)
        finally:
            if timer is not None:
                timer.cancel(); timer.join()
            try:
                if child is not None:
                    stop_owned(child, pidfd)
            finally:
                if pidfd is not None:
                    os.close(pidfd)


def tool_path(name):
    value = shutil.which(name, path=os.defpath)
    require(value, 'tool')
    path = Path(value).resolve(strict=True); info = path.stat()
    require(stat.S_ISREG(info.st_mode) and info.st_uid == 0 and not info.st_mode & 0o022 and info.st_mode & 0o111, 'tool')
    return path


def elf_proof(fd, spool, deadline, name):
    # /proc/self/fd is passed explicitly; the same open ELF is hashed and inspected.
    executable = f'/proc/{os.getpid()}/fd/{fd}'
    notes = run_tool([str(tool_path('readelf')), '-nW', '--debug-dump=no-follow-links', executable], spool, name + '-notes', deadline).decode()
    ids = re.findall(r'Build ID: ([0-9a-f]+)', notes)
    require(len(ids) == 1, 'elf')
    sections = run_tool([str(tool_path('readelf')), '-SW', executable], spool, name + '-sections', deadline).decode()
    found = {}
    for line in sections.splitlines():
        match = re.match(r'\s*\[\s*\d+\]\s+(\.\S+)\s+\S+\s+[0-9a-fA-F]+\s+[0-9a-fA-F]+\s+([0-9a-fA-F]+)\s', line)
        if match:
            found[match[1]] = int(match[2], 16)
    require(all(0 < found.get(key, 0) < CAP for key in ('.symtab', '.eh_frame')), 'elf')
    return ids[0]


def stream_hash(fd, deadline, cap=512 * 1024 * 1024):
    os.lseek(fd, 0, os.SEEK_SET)
    value = hashlib.sha256(); size = 0
    while True:
        require(time.monotonic() <= deadline, 'deadline')
        chunk = os.read(fd, 1024 * 1024)
        if not chunk:
            break
        size += len(chunk); require(size <= cap, 'bytes'); value.update(chunk)
    return value.hexdigest()


def selected_perf(request, deadline):
    path = Path(request['perf_path'])
    require(path.resolve(strict=True) == path and path.name == 'perf', 'tool')
    fd = os.open(path, os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW)
    try:
        before = os.fstat(fd)
        require(stat.S_ISREG(before.st_mode) and before.st_uid == 0 and not before.st_mode & 0o022
                and before.st_mode & 0o111 and 0 < before.st_size <= CAP
                and os.pread(fd, 4, 0) == b'\x7fELF', 'tool')
        require(stream_hash(fd, deadline, CAP) == request['perf_sha256']
                and file_identity(os.fstat(fd)) == file_identity(before), 'tool')
        return path, file_identity(before)
    finally:
        os.close(fd)


def record_args(perf, pid):
    # Linux v5.15 record has no --synth option. Its default synthesis follows the
    # explicit target thread map/mappings; -p, user-only and no-inherit bound samples.
    return [str(perf), 'record', '--event', 'cpu-clock:u', '--freq', '49', '--call-graph', 'dwarf,8192',
            '--timestamp', '--mmap-pages', '128', '--clockid', 'mono', '--no-inherit',
            '--no-buildid-cache', '--no-bpf-event', '--pid', str(pid),
            '--output', 'perf.data']


def census(raw):
    require(104 <= len(raw) <= CAP and raw[:8] == b'PERFILE2', 'census', 1)
    header_size, attr_size, attr_offset, attrs_bytes, offset, size = struct.unpack_from('<6Q', raw, 8)
    require(104 <= header_size <= len(raw) and 48 <= attr_size <= 4096 and attrs_bytes == attr_size
            and attr_offset >= header_size and attr_offset + attr_size <= len(raw)
            and offset >= attr_offset + attr_size and offset + size <= len(raw), 'census', 2)
    kind, length, config, frequency, sample_type, read_format, flags = struct.unpack_from('<II5Q', raw, attr_offset)
    # software cpu-clock, user-only, no inheritance; DWARF requires user regs+stack.
    allowed = sum(1 << bit for bit in (0, 1, 2, 3, 5, 6, 7, 8, 12, 13, 16))
    required = sum(1 << bit for bit in (0, 1, 2, 8, 12, 13))
    # v5.15 clock events have unit="msec", which enables PERF_FORMAT_ID even alone.
    require(kind == 1 and config == 0 and frequency == 49 and read_format == 4
            and 96 <= length <= attr_size, 'census', 3)
    require(not sample_type & ~allowed, 'census', unsupported_sample_detail(sample_type & ~allowed))
    require(sample_type & required == required, 'census', 5)
    require(not flags & (1 << 1) and flags & (1 << 5) and flags & (1 << 6)
            and flags & (1 << 10) and flags & (1 << 25), 'census', 6)
    require(struct.unpack_from('<i', raw, attr_offset + 92)[0] == 1, 'census', 7)
    counts = {'samples': 0, 'lost': 0, 'records': 0}
    end = offset + size
    while offset < end:
        require(offset + 8 <= end, 'census', 8)
        event, misc, length = struct.unpack_from('<IHH', raw, offset)
        require(length >= 8 and offset + length <= end, 'census', 9)
        counts['records'] += 1; require(counts['records'] <= 100000, 'census', 10)
        if event == 9:
            require(misc & 7 == 2, 'census', 11)
            counts['samples'] += 1
        elif event == 79:
            # Linux v5.15 tools/lib/perf/include/perf/event.h: 56-byte time_conv.
            require(length == 56, 'census', 12)
            require(struct.unpack_from('<Q', raw, offset + 8)[0] <= 63, 'census', 13)
            require(raw[offset + 48] in (0, 1) and raw[offset + 49] in (0, 1), 'census', 14)
            require(not any(raw[offset + 50:offset + 56]), 'census', 15)
        elif event in (2, 13):  # LOST / LOST_SAMPLES: any loss makes capture unavailable.
            require(length >= 16, 'census', 16); counts['lost'] += 1
        else:
            require(event in (1, 3, 4, 7, 10, 68, 69, 73, 74, 78, 82), 'census', 17)
        offset += length
    require(counts['samples'] <= SAMPLE_CAP, 'census', 18)
    return counts


def category(symbol, dso, owned, ip=None, executable_maps=()):
    if ip == 0 or symbol in ('[unknown]', 'unknown') or symbol.startswith('[unknown]'):
        return 0
    mapped = ip is not None and any(start <= ip < end for start, end, _ in executable_maps)
    if dso == owned and mapped:
        symbol = symbol[1:] if symbol.startswith('<') else symbol
        if symbol.startswith(('rayon::', 'rayon_core::')):
            return 12
        if (re.match(r'core::ptr::drop_in_place(?:<|::|$)', symbol)
                or re.match(r'alloc::sync::Arc(?:::|<.*>::)drop_slow(?:<|::|$)', symbol)
                or re.match(r'alloc::sync::Arc<.*> as core::ops::drop::Drop>::drop(?:<|::|$)', symbol)
                or symbol.startswith('alloc::alloc::')):
            return 13
        for value, prefixes in ((1, ('light_dynamics::runtime::sampling::',)),
                                (2, ('light_dynamics::programming::preparation::',)),
                                (3, ('light_dynamics::programming::composition::',)),
                                (4, ('light_dynamics::',)), (5, ('light_engine::',)),
                                (6, ('light_headless::', 'light_headless_runtime::')), (7, ('std::', 'core::', 'alloc::'))):
            if any(symbol.startswith(p) for p in prefixes):
                return value
        return 8
    if dso in ('[vdso]', '[vdso32]'):
        return 10
    return 9 if dso.startswith('/') else 11


def decode(text, pid, tids, owned, raw_counts, interval=None, required_names=(), executable_maps=()):
    require(len(text.encode()) <= CAP, 'bytes')
    result = {'samples': 0, 'frames': 0, 'unresolved_self': 0, 'unresolved_ancestor': 0,
              'depth_one': 0, 'owned_rust_chains': 0, 'period': 0,
              'rows': [{'id': n, 'self_samples': 0, 'self_period': 0, 'inclusive_samples': 0, 'inclusive_period': 0} for n in CATEGORIES]}
    current = None; frames = []; useful = 0; first_ns = None; last_ns = None
    header = re.compile(r'^\s*(\d+)\s*/\s*(\d+)\s+(\d+\.\d{1,9}):\s+(\d+)\s+cpu-clock:u:\s*(.*)$')
    frame = re.compile(r'^\s+([0-9a-fA-F]+)\s+(.+)\s+\(([^\r\n\0]+)\)\s*$')
    def finish():
        nonlocal current, frames, useful
        if current is None:
            return
        require(frames and len(frames) <= FRAME_CAP, 'decode')
        result['samples'] += 1; require(result['samples'] <= SAMPLE_CAP, 'decode')
        result['frames'] += len(frames); result['period'] += current
        require(uint(result['period']), 'decode')
        result['depth_one'] += int(len(frames) == 1)
        classes = [category(symbol, dso, owned, ip, executable_maps) for ip, symbol, dso in frames]
        result['unresolved_self'] += int(classes[0] == 0)
        result['unresolved_ancestor'] += classes[1:].count(0)
        rust_ips = {ip for (ip, _, dso), cls in zip(frames, classes) if dso == owned and cls in (*range(1, 8), 12, 13)
                    and any(start <= ip < end for start, end, _ in executable_maps)}
        result['owned_rust_chains'] += int(len(rust_ips) >= 2)
        useful += int(bool(required_names) and all(any(name in symbol for _, symbol, dso in frames if dso == owned) for name in required_names))
        row = result['rows'][classes[0]]; row['self_samples'] += 1; row['self_period'] += current
        for cls in set(classes):
            row = result['rows'][cls]; row['inclusive_samples'] += 1; row['inclusive_period'] += current
        current = None; frames = []
    for line in text.splitlines():
        require(len(line.encode()) <= LINE_CAP, 'decode')
        if not line.strip():
            finish(); continue
        match = header.fullmatch(line)
        if match:
            finish()
            require(int(match[1]) == pid and int(match[2]) in tids and 0 < int(match[4]) <= 2**53 - 1, 'decode')
            seconds, nanos = match[3].split('.')
            timestamp_ns = int(seconds) * 1000000000 + int(nanos.ljust(9, '0'))
            first_ns = timestamp_ns if first_ns is None else min(first_ns, timestamp_ns)
            last_ns = timestamp_ns if last_ns is None else max(last_ns, timestamp_ns)
            timestamp = timestamp_ns / 1000000000
            require(interval is None or interval[0] <= timestamp <= interval[1], 'decode')
            current = int(match[4])
            if match[5]:
                first = frame.fullmatch(' ' + match[5])
                require(first, 'decode')
                frames.append((int(first[1], 16), first[2].strip(), first[3]))
            continue
        match = frame.fullmatch(line)
        require(current is not None and match and len(frames) < FRAME_CAP, 'decode')
        frames.append((int(match[1], 16), match[2].strip(), match[3]))
    finish()
    require(result['samples'] == raw_counts['samples'] and result['samples'] > 0, 'decode')
    require(raw_counts['lost'] == 0, 'loss')
    result['sample_span_micros'] = (last_ns - first_ns + 999) // 1000
    require(not required_names or useful >= 10, 'unwind')
    return result


def capture(perf, pid, tids, owned, spool, duration, deadline, guard, required_names=(), executable_maps=(), stage=None, phases=(0, 0)):
    spool.mkdir(mode=0o700)
    if stage:
        stage(phases[0])
    guard(); begin = time.monotonic()
    window = {}
    run_tool(record_args(perf, pid), spool, 'record', deadline, duration, guard, window)
    end = time.monotonic(); guard()
    raw = bounded_read(spool / 'perf.data'); counts = census(raw)
    require(counts['lost'] == 0, 'loss')
    if stage:
        stage(phases[1])
    text = run_tool([str(perf), 'script', '--ns', '--no-inline', '--demangle', '--show-kernel-path',
                     '-F', 'pid,tid,time,period,event,ip,sym,dso', '-i', 'perf.data'],
                    spool, 'decode', deadline).decode()
    guard()
    report = decode(text, pid, tids, owned, counts, (begin, end), required_names, executable_maps)
    require(report['sample_span_micros'] <= min(10, duration + 2) * 1000000, 'deadline')
    report['recording_envelope_micros'] = round((end - begin) * 1e6)
    report['elapsed_micros'] = round((time.monotonic() - begin) * 1e6)
    return report


PROBE = '''#include <stdint.h>\n#include <stdio.h>\nstatic volatile uint64_t total;\n__attribute__((noinline)) void tl775_owned_probe_leaf(void) { for (uint64_t n=0;n<1000000;n++) total+=n; }\n__attribute__((noinline)) void tl775_owned_probe_parent(void) { for (;;) tl775_owned_probe_leaf(); }\nint main(void) { puts("READY"); fflush(stdout); tl775_owned_probe_parent(); }\n'''


def synthetic(perf, spool, deadline, progress=None):
    stage = spool / 'synthetic'; stage.mkdir(mode=0o700)
    if progress:
        progress(6)
    binary = compile_probe(stage, deadline, True)
    child = None; pidfd = None
    with os.fdopen(private_file(stage / 'probe.stdout'), 'wb') as out, os.fdopen(private_file(stage / 'probe.stderr'), 'wb') as err:
        try:
            child = subprocess.Popen([str(binary)], stdout=out, stderr=err, stdin=subprocess.DEVNULL, env=environment(), umask=0o077)
            pidfd = os.pidfd_open(child.pid)
            if progress:
                progress(14)
            ready_until = min(deadline, time.monotonic() + 2)
            while bounded_read(stage / 'probe.stdout', 64) != b'READY\n':
                require(child.poll() is None and time.monotonic() < ready_until, 'deadline'); time.sleep(.02)
            pinned = identity(child.pid); tasks = threads(child.pid, os.getuid())
            executable = Path('/proc') / str(child.pid) / 'exe'
            owned = os.readlink(executable); info = os.stat(executable)
            _, executable_maps = maps(child.pid, info)
            def guard():
                require(child.poll() is None and identity(child.pid) == pinned
                        and file_identity(os.stat(executable)) == file_identity(info)
                        and os.readlink(executable) == owned, 'identity')
                require(threads(child.pid, os.getuid()) == tasks, 'threads')
            # Also verify controlled parent+leaf, not merely any successful C decode.
            capture(perf, child.pid, tasks, owned, stage / 'capture', 2, deadline, guard,
                    required_names=('tl775_owned_probe_leaf', 'tl775_owned_probe_parent'), stage=progress, phases=(7, 8), executable_maps=executable_maps)
        finally:
            try:
                if child is not None:
                    stop_owned(child, pidfd)
            finally:
                if pidfd is not None:
                    os.close(pidfd)


# The only variant is GCC's root-validated subtool prefix; source/options/TMPDIR stay fixed.
COMPILE_OPTIONS = ('-O1', '-g', '-fno-omit-frame-pointer', '-fno-optimize-sibling-calls')


def compile_probe(stage, deadline, trusted_prefix, temporary=None):
    source = stage / 'probe.c'; source.write_text(PROBE); source.chmod(0o600)
    if temporary is None:
        temporary = stage / 'compiler-tmp'; temporary.mkdir(mode=0o700)
    binary = stage / 'probe'; options = []
    if trusted_prefix:
        assembler, linker = tool_path('as'), tool_path('ld')
        require(assembler.parent == linker.parent, 'tool')
        info = assembler.parent.stat()
        require(stat.S_ISDIR(info.st_mode) and info.st_uid == 0 and not info.st_mode & 0o022, 'tool')
        options = ['-B' + str(assembler.parent) + '/']
    run_tool([str(tool_path('cc')), *options, *COMPILE_OPTIONS, str(source), '-o', str(binary)],
             stage, 'compile', deadline, extra_env={'TMPDIR': str(temporary)})
    info = binary.lstat()
    require(stat.S_ISREG(info.st_mode) and info.st_uid == os.getuid() and info.st_nlink == 1
            and info.st_mode & 0o111 and not info.st_mode & 0o077
            and bounded_read(binary, CAP)[:4] == b'\x7fELF', 'elf')
    return binary


def compiler_failure_kind(stderr):
    # Parse only fixed C-locale error classes. No diagnostic text is ever returned.
    if re.search(r'error while loading shared libraries|cannot open shared object file', stderr):
        return 5
    if re.search(r'Permission denied|Operation not permitted', stderr):
        return 4
    for code, name in ((1, 'as'), (2, 'ld'), (3, 'cc1')):
        if re.search(r"(?:cannot execute|cannot find|unable to execute)\s+['\"`]?[^'\"`\s]*\b" + name + r"(?:['\"`:\s]|$)", stderr):
            return code
    return 6


def validate_compiler_envelope(envelope):
    require(type(envelope) is dict and set(envelope) == {'report', 'provenance'}, 'decode')
    r, provenance = envelope['report'], envelope['provenance']
    require(type(r) is dict and set(r) == {'schema_version', 'diagnostic_mode_code', 'available',
            'source_verified', 'raw_private', 'tools_stopped', 'failure_code', 'attempts'}, 'decode')
    require(all(uint(v) for k, v in r.items() if k != 'attempts') and r['schema_version'] == 1
            and r['diagnostic_mode_code'] == 1 and r['failure_code'] <= len(FAILURES), 'decode')
    require(all(r[k] in (0, 1) for k in ('available', 'source_verified', 'raw_private', 'tools_stopped'))
            and type(r['attempts']) is list and len(r['attempts']) == 2, 'decode')
    for i, row in enumerate(r['attempts']):
        require(type(row) is dict and set(row) == {'id', 'attempted', 'compiled', 'elf_verified',
                'failure_kind', 'tool_failure_code', 'elapsed_micros'} and all(uint(v) for v in row.values())
                and row['id'] == i and all(row[k] in (0, 1) for k in ('attempted', 'compiled', 'elf_verified'))
                and row['failure_kind'] <= 6 and row['tool_failure_code'] <= len(FAILURES)
                and row['compiled'] == row['elf_verified'] <= row['attempted']
                and row['elapsed_micros'] <= 34000000, 'decode')
        require((row['compiled'] and row['failure_kind'] == row['tool_failure_code'] == 0)
                or (not row['compiled'] and row['attempted'] and row['failure_kind'] > 0 and row['tool_failure_code'] > 0)
                or (not row['attempted'] and all(row[k] == 0 for k in ('compiled', 'elf_verified', 'failure_kind', 'tool_failure_code', 'elapsed_micros'))), 'decode')
    require(r['available'] == int(not r['failure_code'] and r['source_verified'] and r['raw_private']
            and r['tools_stopped'] and all(row['attempted'] for row in r['attempts'])), 'decode')
    require(type(provenance) is dict and set(provenance) == {'schema_version', 'candidate_sha', 'helper_sha256'}
            and type(provenance['schema_version']) is int and provenance['schema_version'] == 1, 'decode')
    require(type(provenance['candidate_sha']) is str and re.fullmatch('[0-9a-f]{40}', provenance['candidate_sha'])
            and type(provenance['helper_sha256']) is str and re.fullmatch('[0-9a-f]{64}', provenance['helper_sha256']), 'decode')
    return envelope


def compiler_preflight(candidate, directory):
    require(type(candidate) is str and re.fullmatch('[0-9a-f]{40}', candidate), 'request')
    root = artifact_path('LIGHT_TMP_DIR', 'TMP_ROOT'); spool = Path(directory)
    require(spool.is_absolute() and spool.parent.resolve() == root.resolve()
            and not spool.exists() and not spool.is_symlink(), 'private')
    spool.mkdir(mode=0o700); anchor, spool = anchor_spool(spool)
    r = dict(schema_version=1, diagnostic_mode_code=1, available=0, source_verified=0,
             raw_private=1, tools_stopped=1, failure_code=0,
             attempts=[dict(id=i, attempted=0, compiled=0, elf_verified=0, failure_kind=0,
                            tool_failure_code=0, elapsed_micros=0) for i in range(2)])
    provenance = dict(schema_version=1, candidate_sha=candidate, helper_sha256=digest(bounded_read(__file__)))
    try:
        deadline = time.monotonic() + 30
        argv = [str(tool_path('git')), '-c', 'safe.directory=' + str(ROOT), '-C', str(ROOT), 'rev-parse', 'HEAD']
        require(run_tool(argv, spool, 'source', min(deadline, time.monotonic() + 2)).decode().strip() == candidate, 'source')
        r['source_verified'] = 1
        temporary = spool / 'compiler-tmp'; temporary.mkdir(mode=0o700)
        for row in r['attempts']:
            stage = spool / ('attempt-' + str(row['id'])); stage.mkdir(mode=0o700)
            begin = time.monotonic(); row['attempted'] = 1
            try:
                compile_probe(stage, min(deadline, begin + 10), bool(row['id']), temporary)
                row['compiled'] = row['elf_verified'] = 1
            except (Refused, OSError, ValueError, subprocess.SubprocessError) as error:
                row['tool_failure_code'] = error.code if isinstance(error, Refused) else FAILURES['tool']
                row['failure_kind'] = 6
                stderr = stage / 'compile.stderr'
                row['failure_kind'] = compiler_failure_kind(bounded_read(stderr).decode(errors='replace')) if stderr.is_file() else 6
                if row['tool_failure_code'] == FAILURES['cleanup']:
                    raise Refused('cleanup')
            finally:
                row['elapsed_micros'] = round((time.monotonic() - begin) * 1e6)
        require(run_tool(argv, spool, 'source-after', min(deadline, time.monotonic() + 2)).decode().strip() == candidate
                and digest(bounded_read(__file__)) == provenance['helper_sha256'], 'source')
        r['available'] = 1
    except (Refused, OSError, ValueError, subprocess.SubprocessError) as error:
        r['failure_code'] = error.code if isinstance(error, Refused) else FAILURES['tool']
        r['tools_stopped'] = int(r['failure_code'] != FAILURES['cleanup'])
    finally:
        if anchor is not None:
            os.close(anchor)
    return validate_compiler_envelope(dict(report=r, provenance=provenance))


def validate_numeric(report):
    keys = {'schema_version', 'available', 'failure_code', 'failure_stage_code', 'failure_detail_code', 'synthetic_capable', 'rust_unwind_admitted',
            'ownership_verified', 'identity_verified', 'thread_scope_verified', 'recorders_stopped',
            'raw_private', 'sample_hz', 'requested_seconds', 'preflight_only', 'capture', 'preflight',
            'profile_phase_code', 'exact_measurement_window', 'codegen_flags_changed', 'loss_observed', 'byte_cap_hit'}
    require(type(report) is dict and set(report) == keys, 'decode')
    for key in keys - {'capture', 'preflight'}:
        require(uint(report[key]), 'decode')
    require(report['schema_version'] == 1 and report['sample_hz'] == 49 and report['failure_code'] <= len(FAILURES) and report['failure_stage_code'] <= 14 and report['failure_detail_code'] <= CENSUS_DETAIL_MAX, 'decode')
    require(not report['failure_detail_code'] or (report['failure_code'] == FAILURES['census']
            and report['failure_stage_code'] in (7, 9, 11)), 'decode')
    for key in ('available', 'synthetic_capable', 'rust_unwind_admitted', 'ownership_verified',
                'identity_verified', 'thread_scope_verified', 'recorders_stopped', 'raw_private', 'preflight_only',
                'exact_measurement_window', 'codegen_flags_changed', 'loss_observed', 'byte_cap_hit'):
        require(report[key] in (0, 1), 'decode')
    require(1 <= report['requested_seconds'] <= 10 and report['profile_phase_code'] == 1
            and report['exact_measurement_window'] == report['codegen_flags_changed'] == 0, 'decode')
    capture_keys = {'samples', 'frames', 'unresolved_self', 'unresolved_ancestor', 'depth_one',
                    'owned_rust_chains', 'period', 'rows', 'elapsed_micros', 'recording_envelope_micros', 'sample_span_micros'}
    for name in ('capture', 'preflight'):
        item = report[name]
        if item is None:
            continue
        require(type(item) is dict and set(item) == capture_keys, 'decode')
        require(all(uint(value) for key, value in item.items() if key != 'rows'), 'decode')
        require(item['sample_span_micros'] <= min(10, (2 if name == 'preflight' else report['requested_seconds']) + 2) * 1000000
                and item['sample_span_micros'] <= item['recording_envelope_micros'] + 1
                and item['recording_envelope_micros'] <= item['elapsed_micros'], 'decode')
        n = item['samples']; require(0 < n <= SAMPLE_CAP and n <= item['frames'] <= n * FRAME_CAP
            and item['unresolved_self'] <= n and item['unresolved_ancestor'] <= item['frames'] - n
            and item['depth_one'] <= n and item['owned_rust_chains'] <= n, 'decode')
        require(type(item['rows']) is list and len(item['rows']) == len(CATEGORIES), 'decode')
        for cls, row in enumerate(item['rows']):
            require(type(row) is dict and set(row) == {'id', 'self_samples', 'self_period', 'inclusive_samples', 'inclusive_period'}
                    and all(uint(v) for v in row.values()) and row['id'] == cls
                    and row['self_samples'] <= row['inclusive_samples'] <= n
                    and row['self_period'] <= row['inclusive_period'] <= item['period'], 'decode')
        require(sum(row['self_samples'] for row in item['rows']) == n
                and sum(row['self_period'] for row in item['rows']) == item['period']
                and item['rows'][0]['self_samples'] == item['unresolved_self'], 'decode')
    require(report['rust_unwind_admitted'] == int(report['preflight'] is not None and report['preflight']['owned_rust_chains'] >= 10), 'decode')
    require(report['available'] == int(report['failure_code'] == 0), 'decode')
    if report['available']:
        require(not report['failure_code'] and not report['failure_stage_code'] and all(report[k] for k in ('synthetic_capable', 'rust_unwind_admitted',
                'ownership_verified', 'identity_verified', 'thread_scope_verified', 'recorders_stopped', 'raw_private'))
                and (report['preflight_only'] or report['capture'] is not None), 'decode')
    return report


def write_json(path, value):
    with os.fdopen(private_file(path), 'w') as stream:
        json.dump(value, stream, sort_keys=True); stream.write('\n')


def anchor_spool(path):
    fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
    try:
        info = os.fstat(fd)
        require(info.st_uid == os.getuid() and stat.S_ISDIR(info.st_mode)
                and stat.S_IMODE(info.st_mode) == 0o700, 'private')
        # Parent-owned directories cannot redirect any root write after a rename/symlink swap.
        return fd, Path(f'/proc/{os.getpid()}/fd/{fd}')
    except BaseException:
        os.close(fd); raise


def profile(request, private_directory, preflight_only=False):
    """Return numeric report and hash-only provenance. Caller publishes ONLY these objects.

    No automatic privilege escalation; root invocation requires explicit request opt-in.
    The owned target/root/runner always remain untouched, including all failure paths.
    """
    validate_request(request)
    require(sys.platform == 'linux' and hasattr(os, 'pidfd_open') and hasattr(signal, 'pidfd_send_signal'), 'tool')
    root = artifact_path('LIGHT_TMP_DIR', 'TMP_ROOT')
    spool = Path(private_directory)
    require(spool.is_absolute() and spool.parent.resolve() == root.resolve()
            and not spool.exists() and not spool.is_symlink(), 'private')
    spool.mkdir(mode=0o700)
    anchor, spool = anchor_spool(spool)
    report = {'schema_version': 1, 'available': 0, 'failure_code': 0, 'failure_stage_code': 0, 'failure_detail_code': 0, 'synthetic_capable': 0,
              'rust_unwind_admitted': 0, 'ownership_verified': 0, 'identity_verified': 0,
              'thread_scope_verified': 0, 'recorders_stopped': 0, 'raw_private': 1, 'sample_hz': 49,
              'requested_seconds': request['duration_seconds'], 'preflight_only': int(preflight_only),
              'capture': None, 'preflight': None, 'profile_phase_code': 1, 'exact_measurement_window': 0,
              'codegen_flags_changed': 0, 'loss_observed': 0, 'byte_cap_hit': 0}
    provenance = {'schema_version': 1, 'candidate_sha': request['candidate_sha'],
                  'helper_sha256': digest(bounded_read(__file__)), 'elf_sha256': request['elf_sha256'],
                  'elf_build_id': request['elf_build_id']}
    def stage(value):
        report['failure_stage_code'] = value
    fd = None
    try:
        stage(1)
        overall = time.monotonic() + 90
        actual_source = run_tool([str(tool_path('git')), '-c', 'safe.directory=' + str(ROOT), '-C', str(ROOT), 'rev-parse', 'HEAD'], spool, 'source', min(overall, time.monotonic() + 2)).decode().strip()
        require(actual_source == request['candidate_sha'], 'source')
        stage(2)
        pinned = verify_owner(request); report['ownership_verified'] = 1
        stage(3)
        exe = Path('/proc') / str(request['target_pid']) / 'exe'
        fd = os.open(exe, os.O_RDONLY | os.O_CLOEXEC)
        before = os.fstat(fd)
        require(stat.S_ISREG(before.st_mode) and 0 < before.st_size <= 512 * 1024 * 1024
                and before.st_uid in (0, request['uid']) and not before.st_mode & 0o022, 'elf')
        require(stream_hash(fd, min(overall, time.monotonic() + 8)) == request['elf_sha256'], 'elf')
        link = os.readlink(exe); require(not link.endswith(' (deleted)'), 'identity')
        require(elf_proof(fd, spool, min(overall, time.monotonic() + 8), 'application') == request['elf_build_id'], 'elf')
        stage(4)
        initial_maps, executable_maps = maps(request['target_pid'], before)
        with os.fdopen(private_file(spool / 'maps.before'), 'wb') as stream:
            stream.write(initial_maps)
        def guard():
            require(verify_owner(request) == pinned, 'identity')
            require(file_identity(os.stat(exe)) == file_identity(before)
                    and file_identity(os.fstat(fd)) == file_identity(before) and os.readlink(exe) == link, 'identity')
            require(threads(request['target_pid'], request['uid']) == frozen_threads, 'threads')
            require(maps(request['target_pid'], before)[1] == executable_maps, 'maps')
        stage(5)
        perf, perf_identity = selected_perf(request, min(overall, time.monotonic() + 5))
        provenance['perf_sha256'] = request['perf_sha256']
        synthetic(perf, spool, min(overall, time.monotonic() + 50), stage); report['synthetic_capable'] = 1
        frozen_threads = threads(request['target_pid'], request['uid'])
        stable_until = time.monotonic() + 1
        while time.monotonic() < stable_until:
            guard(); time.sleep(.05)
        guard()
        report['preflight'] = capture(perf, request['target_pid'], frozen_threads, link, spool / 'rust-preflight', 2, min(overall, time.monotonic() + 12), guard, executable_maps=executable_maps, stage=stage, phases=(9, 10))
        require(report['preflight']['owned_rust_chains'] >= 10, 'unwind')
        report['rust_unwind_admitted'] = 1
        if not preflight_only:
            report['capture'] = capture(perf, request['target_pid'], frozen_threads, link, spool / 'capture', request['duration_seconds'], min(overall, time.monotonic() + request['duration_seconds'] + 10), guard, executable_maps=executable_maps, stage=stage, phases=(11, 12))
        stage(13)
        guard()
        require(elf_proof(fd, spool, min(overall, time.monotonic() + 8), 'application-after') == request['elf_build_id'], 'elf')
        require(stream_hash(fd, min(overall, time.monotonic() + 8)) == request['elf_sha256'], 'elf')
        require(selected_perf(request, min(overall, time.monotonic() + 5))[1] == perf_identity, 'tool')
        require(digest(bounded_read(__file__)) == provenance['helper_sha256'], 'source')
        source_after = run_tool([str(tool_path('git')), '-c', 'safe.directory=' + str(ROOT), '-C', str(ROOT), 'rev-parse', 'HEAD'], spool, 'source-after', min(overall, time.monotonic() + 2)).decode().strip()
        require(source_after == request['candidate_sha'], 'source')
        with os.fdopen(private_file(spool / 'maps.after'), 'wb') as stream:
            stream.write(maps(request['target_pid'], before)[0])
        guard(); report['identity_verified'] = report['thread_scope_verified'] = 1
        report['available'] = 1
        report['failure_stage_code'] = 0
    except (Refused, OSError, ValueError, UnicodeError, KeyError, subprocess.SubprocessError) as error:
        report['failure_code'] = error.code if isinstance(error, Refused) else FAILURES['tool']
        report['failure_detail_code'] = error.detail if isinstance(error, Refused) else 0
    finally:
        if fd is not None:
            os.close(fd)
    # Every tool/probe returns only after its own finally has reaped its exact child.
    report['recorders_stopped'] = int(report['failure_code'] != FAILURES['cleanup'])
    report['byte_cap_hit'] = int(report['failure_code'] == FAILURES['bytes'])
    report['loss_observed'] = int(report['failure_code'] == FAILURES['loss'])
    try:
        write_json(spool / 'numeric.json', validate_numeric(report))
        write_json(spool / 'provenance.json', validate_provenance(provenance))
        return report, provenance
    finally:
        if anchor is not None:
            os.close(anchor)


def validate_provenance(value):
    keys = {'schema_version', 'candidate_sha', 'helper_sha256', 'elf_sha256', 'elf_build_id'}
    require(type(value) is dict and set(value) in (keys, keys | {'perf_sha256'})
            and type(value['schema_version']) is int and value['schema_version'] == 1, 'decode')
    for key in set(value) - {'schema_version'}:
        length = '{40}' if key == 'candidate_sha' else '{8,128}' if key == 'elf_build_id' else '{64}'
        require(type(value[key]) is str and re.fullmatch('[0-9a-f]' + length, value[key]), 'decode')
    return value


def validate_envelope(value):
    require(type(value) is dict and set(value) == {'report', 'provenance'}, 'decode')
    validate_numeric(value['report'])
    if value['provenance'] is None:
        require(value['report']['available'] == 0, 'decode')
    else:
        validate_provenance(value['provenance'])
    return value


def failure_report(code):
    return {'schema_version': 1, 'available': 0, 'failure_code': code, 'failure_stage_code': 0, 'failure_detail_code': 0, 'synthetic_capable': 0,
            'rust_unwind_admitted': 0, 'ownership_verified': 0, 'identity_verified': 0,
            'thread_scope_verified': 0, 'recorders_stopped': 0, 'raw_private': 0, 'sample_hz': 49,
            'requested_seconds': 1, 'preflight_only': 0, 'capture': None, 'preflight': None,
            'profile_phase_code': 1, 'exact_measurement_window': 0, 'codegen_flags_changed': 0,
            'loss_observed': 0, 'byte_cap_hit': 0}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--request', type=Path)
    parser.add_argument('--compiler-preflight', action='store_true')
    parser.add_argument('--candidate-sha')
    parser.add_argument('--private-directory', type=Path, required=True)
    parser.add_argument('--preflight-only', action='store_true')
    args = parser.parse_args()
    def cancelled(_signal, _frame):
        raise Refused('deadline')
    signal.signal(signal.SIGTERM, cancelled)
    signal.signal(signal.SIGINT, cancelled)
    if args.compiler_preflight:
        require(args.request is None and not args.preflight_only, 'request')
        envelope = compiler_preflight(args.candidate_sha, args.private_directory)
        print(json.dumps(envelope, sort_keys=True))
        return 0 if envelope['report']['available'] else 1
    require(args.request is not None and args.candidate_sha is None, 'request')
    request = json.loads(bounded_read(args.request, 16384))
    report, provenance = profile(request, args.private_directory, args.preflight_only)
    print(json.dumps(validate_envelope({'report': report, 'provenance': provenance}), sort_keys=True))
    return 0 if report['available'] else 1


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except (Refused, OSError, ValueError, KeyError, UnicodeError) as error:
        # Never print raw paths, tool stderr, symbols, PIDs or arbitrary exceptions.
        print(json.dumps(validate_envelope({'report': failure_report(error.code if isinstance(error, Refused) else FAILURES['tool']),
                                            'provenance': None})))
        raise SystemExit(1)
