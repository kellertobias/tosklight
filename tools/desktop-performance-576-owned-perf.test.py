"""Pure tests only: no perf, compiler, process launch, socket or application."""
import copy
import importlib.util
import json
import os
from pathlib import Path
import struct
import sys
import tempfile
import unittest
from unittest import mock

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).parent))
spec = importlib.util.spec_from_file_location('owned_perf', Path(__file__).with_name('desktop-performance-576-owned-perf.py'))
p = importlib.util.module_from_spec(spec)
spec.loader.exec_module(p)


def request():
    return dict(schema=1, candidate_sha='a' * 40, runner_pid=10, runner_start_ticks=100,
                root_pid=20, root_start_ticks=200, target_pid=30, target_start_ticks=300,
                uid=1000, elf_sha256='b' * 64, elf_build_id='c' * 40, duration_seconds=10,
                allow_root=0, perf_path='/usr/lib/linux-tools/perf', perf_sha256='d' * 64)


def sample(symbol='light_dynamics::runtime::sampling::staged::evaluate', period=7,
           ancestor='light_headless_runtime::runtime::output', ip='a1', ancestor_ip='a2',
           pid=30, tid=31, dso='/owned'):
    return f'{pid}/{tid} 1.234567890: {period} cpu-clock:u: {ip} {symbol} ({dso})\n\t{ancestor_ip} {ancestor} ({dso})\n\n'


def decoded(text=None, n=1, **kwargs):
    return p.decode(text or sample(), 30, {30, 31}, '/owned', {'samples': n, 'lost': 0},
                    interval=(1, 2), executable_maps=[(0xa0, 0xb0, 0)], **kwargs)


def report():
    result = p.failure_report(0)
    result.update(available=1, synthetic_capable=1, rust_unwind_admitted=1,
                  ownership_verified=1, identity_verified=1, thread_scope_verified=1,
                  recorders_stopped=1, raw_private=1, requested_seconds=10)
    result['preflight'] = decoded(sample() * 10, n=10); result['preflight']['elapsed_micros'] = 2000000; result['preflight']['recording_envelope_micros'] = 1900000
    result['capture'] = copy.deepcopy(result['preflight'])
    return result


def raw(events=(9,), sample_type=None, flags=None, read_format=4):
    records = b''.join(struct.pack('<IHH', e, 2, 56 if e == 79 else 16) + b'\0' * (48 if e == 79 else 8) for e in events)
    result = bytearray(232 + len(records)); result[:8] = b'PERFILE2'
    struct.pack_into('<6Q', result, 8, 104, 128, 104, 128, 232, len(records))
    fields = sum(1 << bit for bit in (0, 1, 2, 8, 12, 13)) if sample_type is None else sample_type
    scope = (1 << 5) | (1 << 6) | (1 << 10) | (1 << 25) if flags is None else flags
    struct.pack_into('<II5Q', result, 104, 1, 112, 0, 49, fields, read_format, scope)
    struct.pack_into('<i', result, 104 + 92, 1)
    result[232:] = records
    return bytes(result)


class Pure(unittest.TestCase):
    def setUp(self):
        self.no_launch = mock.patch.object(p.subprocess, 'Popen', side_effect=AssertionError('pure tests must not launch'))
        self.no_launch.start(); self.addCleanup(self.no_launch.stop)

    def refused(self, code, fn, *args, **kwargs):
        with self.assertRaises(p.Refused) as caught:
            fn(*args, **kwargs)
        self.assertEqual(caught.exception.code, p.FAILURES[code])

    def test_exact_request_and_optional_explicit_privileged_invocation(self):
        with mock.patch.object(p.os, 'getuid', return_value=1000):
            self.assertEqual(p.validate_request(request()), request())
            for key, value in [('extra', 1), ('duration_seconds', 11), ('target_pid', True),
                               ('schema', True), ('elf_build_id', '/secret'), ('perf_path', 'perf')]:
                bad = request(); bad[key] = value
                self.refused('request', p.validate_request, bad)
        with mock.patch.object(p.os, 'getuid', return_value=0):
            self.refused('ownership', p.validate_request, request())
            allowed = request(); allowed['allow_root'] = 1
            p.validate_request(allowed)

    def test_owned_runner_ancestry_root_child_and_target_lifetime(self):
        values = {10: (1, 100), 20: (10, 200), 30: (20, 300), 99: (98, 999), 98: (10, 998)}
        def read(pid, *_):
            parent, start = values[pid]
            return dict(pid=pid, ppid=parent, start=start, uid=1000, tgid=pid)
        with mock.patch.object(p, 'identity', side_effect=read), mock.patch.object(p.os, 'getpid', return_value=99):
            self.assertEqual(p.verify_owner(request())['target']['pid'], 30)
            values[20] = (1, 200)
            self.refused('ownership', p.verify_owner, request())
            values[20] = (10, 201)
            self.refused('ownership', p.verify_owner, request())
            values[20] = (10, 200); values[30] = (1, 300)
            self.refused('ownership', p.verify_owner, request())

    def test_cycles_and_bounded_ancestry_are_rejected(self):
        with mock.patch.object(p, 'identity', side_effect=lambda pid, *_: {'ppid': pid}):
            self.assertFalse(p.ancestry(20, 10))

    def test_task_stat_last_parenthesis_and_startticks(self):
        with tempfile.TemporaryDirectory(dir=p.artifact_path('LIGHT_TMP_DIR', 'TMP_ROOT')) as folder:
            proc = Path(folder); directory = proc / '30'; directory.mkdir()
            fields = ['S', '20'] + ['0'] * 17 + ['300']
            (directory / 'stat').write_text('30 (name ) with spaces) ' + ' '.join(fields))
            (directory / 'status').write_text('Uid:\t1000 1000 1000 1000\nTgid:\t30\n')
            self.assertEqual(p.identity(30, proc), dict(pid=30, ppid=20, start=300, uid=1000, tgid=30))

    def test_dwarf_single_pid_command_never_global_or_inherited(self):
        args = p.record_args('/usr/lib/linux-tools/perf', 30)
        self.assertEqual(args[args.index('--pid') + 1], '30')
        self.assertEqual(args[args.index('--call-graph') + 1], 'dwarf,8192')
        self.assertIn('--no-inherit', args)
        self.assertFalse(any(arg == '--synth' or arg.startswith('--synth=') for arg in args))
        self.assertEqual(args[args.index('--event') + 1], 'cpu-clock:u')
        for forbidden in ('-a', '--all-cpus', '-C', '--all-kernel', 'sleep'):
            self.assertNotIn(forbidden, args)
        self.assertNotIn('HOME', p.environment())
        self.assertEqual(p.environment()['DEBUGINFOD_URLS'], '')

    def test_system_tool_requires_root_owner_and_nonwritable_mode(self):
        path = mock.Mock(); path.resolve.return_value = path
        info = mock.Mock(st_mode=p.stat.S_IFREG | 0o755, st_uid=0); path.stat.return_value = info
        with mock.patch.object(p.shutil, 'which', return_value='/usr/bin/readelf'), \
             mock.patch.object(p, 'Path', return_value=path):
            self.assertIs(p.tool_path('readelf'), path)
            info.st_uid = 1000
            self.refused('tool', p.tool_path, 'readelf')
            info.st_uid = 0; info.st_mode = p.stat.S_IFREG | 0o777
            self.refused('tool', p.tool_path, 'readelf')

    def test_perf_header_scope_and_loss_inventory(self):
        self.assertEqual(p.census(raw((9, 9, 2, 79)))['lost'], 1)
        for value in (raw(flags=0), raw(flags=(1 << 5) | (1 << 6) | 2),
                      raw(sample_type=1 << 8), raw((999,)), raw((80,)), raw()[:-1], b'bad'):
            self.refused('census', p.census, value)
        self.refused('census', p.census, raw((9,) * (p.SAMPLE_CAP + 1)))

    def test_single_clock_event_requires_exact_perf_format_id(self):
        self.assertEqual(p.census(raw(read_format=4)), dict(samples=1, lost=0, records=1))
        # Zero, timing, group, lost and unknown flags are not this clock event's format.
        for read_format in (0, 1, 2, 3, 5, 6, 7, 8, 12, 16, 20, 32, 2**63):
            with self.subTest(read_format=read_format), self.assertRaises(p.Refused) as caught:
                p.census(raw(read_format=read_format))
            self.assertEqual((caught.exception.code, caught.exception.detail), (11, 3))

    def test_census_first_failed_predicates_have_only_fixed_details(self):
        def changed(data, offset, fmt, value):
            data = bytearray(data); struct.pack_into(fmt, data, offset, value)
            return bytes(data)
        short_record = bytearray(raw()); struct.pack_into('<Q', short_record, 48, 1)
        short_record = bytes(short_record[:233])
        required = sum(1 << bit for bit in (0, 1, 2, 8, 12, 13))
        cases = [
            (b'bad', 1), (changed(raw(), 32, '<Q', 129), 2),
            (changed(raw(), 104, '<I', 2), 3), (changed(raw(), 136, '<Q', 0), 3),
            (raw(sample_type=required | (1 << 63)), 4), (raw(sample_type=required & ~1), 5),
            (raw(flags=0), 6), (changed(raw(), 196, '<i', 0), 7),
            (short_record, 8), (changed(raw(), 238, '<H', 7), 9),
            (raw((1,) * 100001), 10), (changed(raw(), 236, '<H', 1), 11),
            (changed(raw((79,)), 238, '<H', 55), 12),
            (changed(raw((79,)), 240, '<Q', 64), 13),
            (changed(raw((79,)), 280, '<B', 2), 14),
            (changed(raw((79,)), 282, '<B', 1), 15),
            (changed(raw((2,)), 238, '<H', 8), 16), (raw((999,)), 17),
            (raw((9,) * (p.SAMPLE_CAP + 1)), 18),
        ]
        for data, detail in cases:
            with self.subTest(detail=detail), self.assertRaises(p.Refused) as caught:
                p.census(data)
            self.assertEqual(caught.exception.code, 11)
            self.assertEqual(caught.exception.detail, detail)
            self.assertEqual(str(caught.exception), 'census')
        self.assertEqual(p.census(raw((1, 3, 4, 7, 10, 68, 69, 73, 74, 78, 82, 79, 9, 2, 13))),
                         dict(samples=1, lost=2, records=15))

    def test_unsupported_sample_bits_have_fixed_names_without_admission_changes(self):
        required = sum(1 << bit for bit in (0, 1, 2, 8, 12, 13))
        # Independent UAPI positions, not the implementation's mapping table.
        bits = (4, 9, 10, 11, 14, 15, 17, 18, 19, 20, 21, 22, 23, 24)
        for index, bit in enumerate(bits):
            if bit == 15:  # Reserved detail24 remains stable, but DWARF now admits DATA_SRC.
                continue
            for extra in (0, 1 << 63, 1 << 24):
                with self.subTest(bit=bit, extra=extra), self.assertRaises(p.Refused) as caught:
                    p.census(raw(sample_type=required | (1 << bit) | extra))
                self.assertEqual((caught.exception.code, caught.exception.detail), (11, 19 + index))
        with self.assertRaises(p.Refused) as caught:
            p.census(raw(sample_type=required | (1 << 63)))
        self.assertEqual((caught.exception.code, caught.exception.detail), (11, 4))
        # DWARF's source-proven sample_address callback requests ADDR+DATA_SRC.
        self.assertEqual(p.census(raw(sample_type=required | (1 << 3) | (1 << 15)))['samples'], 1)
        self.assertEqual(p.unsupported_sample_detail(1 << 15), 24)
        # Required/allowed fields still succeed, and the earlier attr predicate wins.
        self.assertEqual(p.census(raw(sample_type=required))['samples'], 1)
        with self.assertRaises(p.Refused) as caught:
            p.census(raw(sample_type=required | (1 << 4), read_format=0))
        self.assertEqual(caught.exception.detail, 3)

    def test_census_details_require_census_failure_and_recording_stage(self):
        for detail in range(1, p.CENSUS_DETAIL_MAX + 1):
            for stage in (7, 9, 11):
                value = p.failure_report(11)
                value.update(failure_stage_code=stage, failure_detail_code=detail)
                p.validate_numeric(value)
        for code, stage, detail in ((0, 0, 1), (7, 7, 1), (11, 6, 1), (11, 7, 33),
                                    (11, 7, -1), (11, 7, 0.5), (11, 7, True)):
            value = p.failure_report(code)
            value.update(failure_stage_code=stage, failure_detail_code=detail)
            self.refused('decode', p.validate_numeric, value)
        self.assertEqual(p.failure_report(1)['failure_detail_code'], 0)
        with self.assertRaises(AssertionError):
            p.Refused('tool', 1)

    def test_profile_census_catch_retains_stage_and_fixed_detail(self):
        with tempfile.TemporaryDirectory(dir=p.artifact_path('LIGHT_TMP_DIR', 'TMP_ROOT')) as folder, \
             tempfile.TemporaryFile(dir=p.artifact_path('LIGHT_TMP_DIR', 'TMP_ROOT')) as elf:
            spool = Path(folder); spool.rmdir()
            real_open, real_fstat, real_readlink = p.os.open, p.os.fstat, p.os.readlink
            bound = []
            def opened(file, *args, **kwargs):
                if str(file) == '/proc/30/exe':
                    fd = os.dup(elf.fileno()); bound.append(fd); return fd
                return real_open(file, *args, **kwargs)
            def inspected(fd):
                return mock.Mock(st_mode=p.stat.S_IFREG | 0o600, st_size=4, st_uid=1000) if fd in bound else real_fstat(fd)
            def synthetic(_perf, _spool, _deadline, progress):
                progress(7)
                raise p.Refused('census', 17)
            with mock.patch.object(p.os, 'getuid', return_value=1000), \
                 mock.patch.object(p.sys, 'platform', 'linux'), \
                 mock.patch.object(p.os, 'pidfd_open', create=True), \
                 mock.patch.object(p.signal, 'pidfd_send_signal', create=True), \
                 mock.patch.object(p, 'run_tool', return_value=b'a' * 40 + b'\n'), \
                 mock.patch.object(p, 'tool_path', return_value=Path('/git')), \
                 mock.patch.object(p, 'verify_owner', return_value={}), \
                 mock.patch.object(p, 'anchor_spool', return_value=(None, spool)), \
                 mock.patch.object(p.os, 'open', side_effect=opened), \
                 mock.patch.object(p.os, 'fstat', side_effect=inspected), \
                 mock.patch.object(p.os, 'readlink', side_effect=lambda file, *a, **kw: '/owned' if str(file) == '/proc/30/exe' else real_readlink(file, *a, **kw)), \
                 mock.patch.object(p, 'stream_hash', return_value='b' * 64), \
                 mock.patch.object(p, 'elf_proof', return_value='c' * 40), \
                 mock.patch.object(p, 'maps', return_value=(b'bounded', [(0xa0, 0xb0, 0)])), \
                 mock.patch.object(p, 'selected_perf', return_value=(Path('/perf'), ())), \
                 mock.patch.object(p, 'synthetic', side_effect=synthetic):
                value, provenance = p.profile(request(), spool)
            self.assertEqual((value['failure_code'], value['failure_stage_code'], value['failure_detail_code']), (11, 7, 17))
            self.assertEqual(value['available'], 0)
            p.validate_envelope(dict(report=value, provenance=provenance))

    def test_weighted_self_inclusive_and_owned_rust_multiframe(self):
        result = decoded(sample(period=7) + sample(symbol='light_engine::render', period=11), n=2)
        self.assertEqual((result['samples'], result['period'], result['frames']), (2, 18, 4))
        self.assertEqual(result['rows'][1]['self_period'], 7)
        self.assertEqual(result['rows'][5]['self_period'], 11)
        self.assertEqual(result['rows'][6]['inclusive_period'], 18)
        self.assertEqual(result['owned_rust_chains'], 2)

    def test_alias_or_outside_owned_mapping_does_not_prove_source_ownership(self):
        for text in (sample(dso='/alias'), sample(ip='f1', ancestor_ip='f2')):
            result = decoded(text)
            self.assertEqual(result['owned_rust_chains'], 0)
            self.assertEqual(result['rows'][9]['self_samples'], 1)
        self.assertEqual(decoded(sample(ancestor_ip='a1'))['owned_rust_chains'], 0)

    def test_fixed_rayon_drop_and_trait_namespace_boundaries(self):
        mapped = [(0xa0, 0xb0, 0)]
        cases = {'rayon_core::job::run': 12, '<rayon::iter::Map as Trait>::go': 12,
                 'rayon_core_fake::job': 8, '<<rayon_core::job': 8,
                 'core::ptr::drop_in_place<Thing>': 13, 'alloc::alloc::dealloc': 13,
                 'alloc::sync::Arc<Thing>::clone': 7, 'alloc::sync::Arc<Thing>::drop_slow': 13,
                 '<alloc::sync::Arc<Thing> as core::ops::drop::Drop>::drop': 13, 'core::ptr::drop_in_place_fake': 7, 'std::thread::park': 7, 'light_dynamics_fake::runtime': 8,
                 'light_headless_runtime::runtime::run': 6}
        for symbol, cls in cases.items():
            self.assertEqual(p.category(symbol, '/owned', '/owned', 0xa1, mapped), cls)
        self.assertEqual(p.category('malloc', '/lib/libc.so', '/owned', 0xa1, mapped), 9)
        self.assertEqual(p.category('malloc', '[vdso]', '/owned', 0xa1, mapped), 10)

    def test_zero_ip_unknown_and_shallow_chains_remain_counted(self):
        text = '30/31 1.234: 3 cpu-clock:u: 0 [unknown] ([unknown])\n\n'
        result = decoded(text)
        self.assertEqual((result['samples'], result['depth_one'], result['unresolved_self']), (1, 1, 1))
        result = decoded(sample(ancestor='[unknown]', ancestor_ip='0'))
        self.assertEqual(result['unresolved_ancestor'], 1)
        self.assertEqual(result['owned_rust_chains'], 0)

    def test_strict_decode_identity_period_interval_census_and_lines(self):
        for text in (sample(pid=99), sample(tid=99), sample(period=0),
                     sample().replace('1.234567890', '9.234567890'), sample() + '# ignored\n',
                     sample(symbol='x' * (p.LINE_CAP + 1))):
            self.refused('decode', decoded, text)
        self.refused('decode', decoded, sample(), n=2)
        self.refused('decode', decoded, sample(period=2**53))
        self.refused('loss', p.decode, sample(), 30, {31}, '/owned', {'samples': 1, 'lost': 1})

    def test_stack_depth_cap_and_period_sum_overflow(self):
        text = sample().rstrip() + '\n' + '\ta3 foo (/owned)\n' * p.FRAME_CAP
        self.refused('decode', decoded, text)
        self.refused('decode', decoded, sample(period=2**53 - 1) + sample(), n=2)

    def test_controlled_parent_leaf_must_cooccur_in_ten_samples(self):
        valid = sample(symbol='tl775_owned_probe_leaf', ancestor='tl775_owned_probe_parent')
        decoded(valid * 10, n=10, required_names=('tl775_owned_probe_leaf', 'tl775_owned_probe_parent'))
        self.refused('unwind', decoded, valid, required_names=('tl775_owned_probe_leaf', 'tl775_owned_probe_parent'))
        separate = sample(symbol='tl775_owned_probe_leaf') + sample(symbol='tl775_owned_probe_parent')
        self.refused('unwind', decoded, separate * 10, n=20,
                     required_names=('tl775_owned_probe_leaf', 'tl775_owned_probe_parent'))

    def test_section_sizes_and_single_build_id_required(self):
        with mock.patch.object(p, 'tool_path', return_value=Path('/readelf')):
            sections = b' [1] .symtab SYMTAB 00000000 000100 000080 18 0 0 8\n [2] .eh_frame PROGBITS 00000000 000200 000040 00 A 0 0 8\n'
            for notes, section, good in ((b'Build ID: abcdef12\n', sections, True),
                                         (b'Build ID: abcdef12\nBuild ID: abcdef12\n', sections, False),
                                         (b'Build ID: abcdef12\n', sections.replace(b'000040', b'000000'), False)):
                with mock.patch.object(p, 'run_tool', side_effect=[notes, section]) as tools:
                    if good:
                        self.assertEqual(p.elf_proof(5, Path('/private'), 999, 'test'), 'abcdef12')
                        self.assertIn('--debug-dump=no-follow-links', tools.call_args_list[0].args[0])
                    else:
                        self.refused('elf', p.elf_proof, 5, Path('/private'), 999, 'test')

    def test_bounded_stream_hash_python310_and_inode_precision(self):
        with tempfile.TemporaryDirectory(dir=p.artifact_path('LIGHT_TMP_DIR', 'TMP_ROOT')) as folder:
            path = Path(folder) / 'data'; path.write_bytes(b'abc')
            with open(path, 'rb') as stream:
                self.assertEqual(p.stream_hash(stream.fileno(), p.time.monotonic() + 1), p.digest(b'abc'))
                self.refused('bytes', p.stream_hash, stream.fileno(), p.time.monotonic() + 1, 2)
                self.refused('deadline', p.stream_hash, stream.fileno(), 0)
        fake = mock.Mock(st_dev=1, st_ino=2**63 + 1, st_size=10, st_mtime_ns=20, st_ctime_ns=21)
        self.assertEqual(p.file_identity(fake)[1], 2**63 + 1)

    def test_closed_envelope_shape_bounds_and_no_identity_strings(self):
        r = report(); p.validate_numeric(r)
        provenance = dict(schema_version=1, candidate_sha='a' * 40, helper_sha256='b' * 64,
                          elf_sha256='c' * 64, elf_build_id='d' * 40, perf_sha256='e' * 64)
        p.validate_envelope(dict(report=r, provenance=provenance))
        bad = copy.deepcopy(provenance); bad['path'] = '/private/symbol'
        self.refused('decode', p.validate_provenance, bad)
        for mutate in (lambda x: x.update(pid=30), lambda x: x.update(failure_code='raw error'),
                       lambda x: x.update(exact_measurement_window=1),
                       lambda x: x['capture']['rows'][1].update(self_samples=0),
                       lambda x: x['capture']['rows'][1].update(self_period=2**53),
                       lambda x: x.update(recorders_stopped=0),
                       lambda x: x.update(rust_unwind_admitted=0)):
            bad = copy.deepcopy(r); mutate(bad)
            self.refused('decode', p.validate_numeric, bad)
        encoded = json.dumps(p.validate_envelope(dict(report=p.failure_report(1), provenance=None)))
        self.assertNotIn('/private', encoded)

    def test_tool_guard_failure_reaps_child_and_closes_pidfd(self):
        with tempfile.TemporaryDirectory(dir=p.artifact_path('LIGHT_TMP_DIR', 'TMP_ROOT')) as folder:
            spool = Path(folder); child = mock.Mock(); child.poll.return_value = None
            pidfd = os.open('/dev/null', os.O_RDONLY)
            def guard():
                raise p.Refused('identity')
            with mock.patch.object(p.subprocess, 'Popen', return_value=child), \
                 mock.patch.object(p.os, 'pidfd_open', return_value=pidfd, create=True), \
                 mock.patch.object(p, 'stop_owned', return_value=0) as stop:
                self.refused('identity', p.run_tool, ['NOT_EXECUTED'], spool, 'owned', p.time.monotonic() + 1, guard=guard)
                stop.assert_called_once_with(child, pidfd)
                with self.assertRaises(OSError):
                    os.fstat(pidfd)
                self.assertEqual((spool / 'owned.stdout').stat().st_mode & 0o777, 0o600)

    def test_profile_failure_retains_only_closed_numeric_and_private_files(self):
        with tempfile.TemporaryDirectory(dir=p.artifact_path('LIGHT_TMP_DIR', 'TMP_ROOT')) as folder:
            spool = Path(folder); spool.rmdir()
            with mock.patch.object(p.os, 'getuid', return_value=1000), \
                 mock.patch.object(p.sys, 'platform', 'linux'), \
                 mock.patch.object(p.os, 'pidfd_open', create=True), \
                 mock.patch.object(p.signal, 'pidfd_send_signal', create=True), \
                 mock.patch.object(p, 'run_tool', return_value=b'a' * 40 + b'\n'), \
                 mock.patch.object(p, 'tool_path', return_value=Path('/git')), \
                 mock.patch.object(p, 'verify_owner', side_effect=p.Refused('ownership')), \
                 mock.patch.object(p, 'anchor_spool', return_value=(None, spool)):
                numeric, provenance = p.profile(request(), spool)
            p.validate_envelope(dict(report=numeric, provenance=provenance))
            self.assertEqual(numeric['failure_code'], p.FAILURES['ownership'])
            self.assertEqual(numeric['failure_detail_code'], 0)
            self.assertEqual(numeric['available'], 0)
            self.assertEqual(spool.stat().st_mode & 0o777, 0o700)
            self.assertEqual({file.name for file in spool.iterdir()}, {'numeric.json', 'provenance.json'})
            self.assertTrue(all(file.stat().st_mode & 0o777 == 0o600 for file in spool.iterdir()))

    def test_capture_argv_long_dso_path_and_duration_interval(self):
        with tempfile.TemporaryDirectory(dir=p.artifact_path('LIGHT_TMP_DIR', 'TMP_ROOT')) as folder:
            output = Path(folder) / 'capture'
            def tool(argv, spool, name, deadline, seconds=None, guard=None, window=None):
                if name == 'record':
                    window['begin'] = 1.0
                    return b''
                self.assertIn('--show-kernel-path', argv)
                self.assertNotIn('--full-paths', argv)
                return sample().encode()
            with mock.patch.object(p, 'run_tool', side_effect=tool), \
                 mock.patch.object(p, 'bounded_read', return_value=raw()), \
                 mock.patch.object(p.time, 'monotonic', side_effect=[1.0, 2.0, 3.0]):
                r = p.capture('/perf', 30, {31}, '/owned', output, 2, 100, lambda: None,
                              executable_maps=[(0xa0, 0xb0, 0)])
                self.assertEqual(r['samples'], 1)

    def test_time_conv_fixed_structural_bounds(self):
        good = bytearray(raw((79, 9)))
        self.assertEqual(p.census(good)['samples'], 1)
        for index, value in ((232 + 8, 64), (232 + 48, 2), (232 + 50, 1)):
            bad = bytearray(good); bad[index] = value
            self.refused('census', p.census, bad)

    def test_intentional_perf_sigint_is_allowed_only_after_our_timer(self):
        with tempfile.TemporaryDirectory(dir=p.artifact_path('LIGHT_TMP_DIR', 'TMP_ROOT')) as folder:
            for timed, code, accepted in ((True, -p.signal.SIGINT, True), (False, -p.signal.SIGINT, False),
                                          (True, -p.signal.SIGTERM, False)):
                spool = Path(folder) / str(int(timed)) / str(code); spool.mkdir(parents=True)
                child = mock.Mock(); child.poll.side_effect = [None, code]; child.returncode = code
                pidfd = os.open('/dev/null', os.O_RDONLY)
                def timer(_seconds, callback):
                    value = mock.Mock(); value.start.side_effect = callback
                    return value
                with mock.patch.object(p.subprocess, 'Popen', return_value=child), \
                     mock.patch.object(p.os, 'pidfd_open', return_value=pidfd, create=True), \
                     mock.patch.object(p.signal, 'pidfd_send_signal', create=True), \
                     mock.patch.object(p.threading, 'Timer', side_effect=timer), \
                     mock.patch.object(p, 'stop_owned', return_value=code):
                    if accepted:
                        self.assertEqual(p.run_tool(['NOT_EXECUTED'], spool, 'record', p.time.monotonic() + 1, seconds=1), b'')
                    else:
                        self.refused('tool', p.run_tool, ['NOT_EXECUTED'], spool, 'record', p.time.monotonic() + 1, seconds=1 if timed else None)

    def test_signal_and_wait_errors_are_cleanup_failure_not_tool_success(self):
        for field in ('signal', 'wait'):
            child = mock.Mock(); child.poll.return_value = None
            with mock.patch.object(p.signal, 'pidfd_send_signal', create=True) as send, \
                 mock.patch.object(p.signal, 'pthread_sigmask', return_value=set()):
                if field == 'signal':
                    send.side_effect = PermissionError('PRIVATE raw failure')
                else:
                    child.wait.side_effect = OSError('PRIVATE raw failure')
                self.refused('cleanup', p.stop_owned, child, 77)

    def test_compiler_failure_classes_never_return_private_text(self):
        for text, kind in (("cc: cannot execute 'as': execvp: No such file", 1),
                           ("collect2: cannot find 'ld'", 2), ("cc: cannot execute 'cc1'", 3),
                           ('PRIVATE/path: Permission denied', 4),
                           ('error while loading shared libraries: PRIVATE/path.so', 5), ('PRIVATE unknown', 6)):
            self.assertEqual(p.compiler_failure_kind(text), kind)
        self.assertEqual(p.compiler_failure_kind('ld: cannot find -lc'), 6)

    def test_compile_probe_uses_verified_prefix_and_private_tmp_without_execution(self):
        with tempfile.TemporaryDirectory(dir=p.artifact_path('LIGHT_TMP_DIR', 'TMP_ROOT')) as folder:
            stage = Path(folder); parent = mock.Mock(); parent.stat.return_value = mock.Mock(st_mode=p.stat.S_IFDIR | 0o755, st_uid=0)
            subtool = mock.Mock(); subtool.parent = parent
            def tool(name):
                return subtool if name in ('as', 'ld') else Path('/cc')
            def compile_only(argv, spool, name, deadline, **kwargs):
                self.assertTrue(argv[1].startswith('-B'))
                self.assertEqual(tuple(argv[2:6]), p.COMPILE_OPTIONS)
                self.assertEqual(set(kwargs['extra_env']), {'TMPDIR'})
                self.assertEqual(Path(kwargs['extra_env']['TMPDIR']).parent, stage)
                binary = stage / 'probe'; binary.write_bytes(b'\x7fELFfake'); binary.chmod(0o700)
            with mock.patch.object(p, 'tool_path', side_effect=tool), mock.patch.object(p, 'run_tool', side_effect=compile_only):
                self.assertEqual(p.compile_probe(stage, 100, True), stage / 'probe')

    def test_compiler_mode_two_compile_only_attempts_and_closed_result(self):
        with tempfile.TemporaryDirectory(dir=p.artifact_path('LIGHT_TMP_DIR', 'TMP_ROOT')) as folder:
            spool = Path(folder); spool.rmdir(); calls = []
            def compile_only(stage, deadline, trusted, temporary):
                calls.append((trusted, temporary))
                if not trusted:
                    (stage / 'compile.stderr').write_text("cc: cannot execute 'as': PRIVATE/path")
                    raise p.Refused('tool')
            with mock.patch.object(p, 'anchor_spool', return_value=(None, spool)), \
                 mock.patch.object(p, 'tool_path', return_value=Path('/git')), \
                 mock.patch.object(p, 'run_tool', return_value=b'a' * 40), \
                 mock.patch.object(p, 'compile_probe', side_effect=compile_only):
                envelope = p.compiler_preflight('a' * 40, spool)
            self.assertEqual([v[0] for v in calls], [False, True])
            self.assertEqual(calls[0][1], calls[1][1])
            self.assertEqual(envelope['report']['available'], 1)
            self.assertEqual([v['compiled'] for v in envelope['report']['attempts']], [0, 1])
            self.assertEqual(envelope['report']['attempts'][0]['failure_kind'], 1)
            self.assertNotIn('PRIVATE', json.dumps(envelope))
            for mutate in (lambda e: e['report'].update(raw='PRIVATE'),
                           lambda e: e['report']['attempts'][1].update(failure_kind=1),
                           lambda e: e['provenance'].update(path='PRIVATE')):
                bad = copy.deepcopy(envelope); mutate(bad)
                self.refused('decode', p.validate_compiler_envelope, bad)

    def test_compiler_cleanup_failure_prevents_second_attempt(self):
        with tempfile.TemporaryDirectory(dir=p.artifact_path('LIGHT_TMP_DIR', 'TMP_ROOT')) as folder:
            spool = Path(folder); spool.rmdir()
            with mock.patch.object(p, 'anchor_spool', return_value=(None, spool)), \
                 mock.patch.object(p, 'tool_path', return_value=Path('/git')), \
                 mock.patch.object(p, 'run_tool', return_value=b'a' * 40), \
                 mock.patch.object(p, 'compile_probe', side_effect=p.Refused('cleanup')) as compile_only:
                r = p.compiler_preflight('a' * 40, spool)['report']
            self.assertEqual(compile_only.call_count, 1)
            self.assertEqual((r['available'], r['tools_stopped'], r['failure_code']), (0, 0, p.FAILURES['cleanup']))

    def test_owned_cleanup_escalates_only_same_pidfd_and_waits(self):
        child = mock.Mock(); child.poll.side_effect = [None, None, None]
        child.wait.side_effect = [p.subprocess.TimeoutExpired('owned', 2), p.subprocess.TimeoutExpired('owned', 1), 0]
        with mock.patch.object(p.signal, 'pidfd_send_signal', create=True) as send, mock.patch.object(p.signal, 'pthread_sigmask', return_value=set()):
            self.assertEqual(p.stop_owned(child, 77), 0)
            self.assertEqual([c.args for c in send.call_args_list], [(77, p.signal.SIGINT), (77, p.signal.SIGTERM), (77, p.signal.SIGKILL)])
            child.kill.assert_not_called()
        child = mock.Mock(); child.poll.return_value = None
        child.wait.side_effect = p.subprocess.TimeoutExpired('owned', 1)
        with mock.patch.object(p.signal, 'pidfd_send_signal', create=True), mock.patch.object(p.signal, 'pthread_sigmask', return_value=set()):
            self.refused('cleanup', p.stop_owned, child, 77)


    def test_synthetic_mode_reuses_fixed_probe_and_has_no_app_claim(self):
        for failure in (None, p.Refused('census', 20), p.Refused('cleanup'), p.Refused('deadline')):
            with tempfile.TemporaryDirectory(dir=p.artifact_path('LIGHT_TMP_DIR', 'TMP_ROOT')) as folder:
                spool = Path(folder) / 'private'; body = Path(p.__file__).read_bytes()
                def run(argv, *_args, **_kwargs):
                    if argv[-1] == '--version': return b'perf version 6.8.0-102-generic\n'
                    if 'show' in argv: return body
                    return b'a' * 40 + b'\n'
                def probe(_perf, _spool, deadline, progress):
                    self.assertLessEqual(deadline - p.time.monotonic(), 30)
                    progress(7)
                    if failure: raise failure
                with mock.patch.object(p.sys, 'platform', 'linux'), mock.patch.object(p.os, 'pidfd_open', create=True), \
                     mock.patch.object(p.signal, 'pidfd_send_signal', create=True), \
                     mock.patch.object(p, 'artifact_path', return_value=Path(folder)), \
                     mock.patch.object(p, 'anchor_spool', side_effect=lambda path:(None,path)), \
                     mock.patch.object(p, 'tool_path', return_value=Path('/usr/bin/git')), \
                     mock.patch.object(p, 'run_tool', side_effect=run), \
                     mock.patch.object(p, 'selected_perf', return_value=(Path('/trusted/perf'),(1,2))), \
                     mock.patch.object(p, 'synthetic', side_effect=probe) as synthetic:
                    envelope = p.synthetic_preflight('a'*40, '/trusted/perf', 'b'*64, spool)
                r = envelope['report'];self.assertEqual(synthetic.call_count,1)
                self.assertEqual((r['perf_major'],r['perf_minor']),(6,8))
                self.assertEqual((r['app_admission'],r['rust_admission'],r['cadence_proof']),(0,0,0))
                self.assertEqual(r['available'],int(failure is None));self.assertEqual(r['synthetic_capable'],int(failure is None))
                self.assertEqual(r['tools_stopped'],int(failure is None or failure.code!=14))
                self.assertEqual(r['failure_detail_code'],20 if failure and failure.code==11 else 0)
                self.assertEqual(r['raw_private'],1);p.validate_synthetic_envelope(envelope)

    def test_synthetic_closed_schema_version_and_failure_coherence(self):
        r=dict.fromkeys(p.SYNTHETIC_KEYS,0);r.update(schema_version=1,diagnostic_mode_code=2,failure_code=7,tools_stopped=1)
        envelope=dict(report=r,provenance=dict(schema_version=1,candidate_sha='a'*40,helper_sha256='b'*64,perf_sha256='c'*64))
        p.validate_synthetic_envelope(envelope)
        for key,value in [('private_path','secret'),('available',1),('app_admission',1),('rust_admission',1),('cadence_proof',1),('failure_detail_code',20),('perf_major',6),('tools_stopped',0),('elapsed_micros',46000001)]:
            bad=copy.deepcopy(envelope);bad['report'][key]=value
            self.refused('decode',p.validate_synthetic_envelope,bad)
        bad=copy.deepcopy(envelope);bad['provenance']['raw']='secret';self.refused('decode',p.validate_synthetic_envelope,bad)
        for text in (b'perf version 5.15\n',b'perf version 6.8.0-102-generic\n',b'perf version 6.0\n'):
            self.assertGreater(p.perf_version(text)[0],0)
        for text in (b'perf version 0.1',b'perf version secret/path',b'perf version 6.8\nprivate',b'x'*257):
            self.refused('tool',p.perf_version,text)

    def test_synthetic_cli_is_explicit_and_disjoint_from_existing_modes(self):
        value=dict(report={'available':0},provenance={})
        args=['helper','--synthetic-preflight','--candidate-sha','a'*40,'--perf-path','/trusted/perf','--perf-sha256','b'*64,'--private-directory','/private']
        with mock.patch.object(p.sys,'argv',args),mock.patch.object(p,'synthetic_preflight',return_value=value) as call,mock.patch('builtins.print'):
            self.assertEqual(p.main(),1);self.assertEqual(call.call_count,1)
        for extra in (['--compiler-preflight'],['--preflight-only'],['--request','/secret']):
            with mock.patch.object(p.sys,'argv',args+extra):self.refused('request',p.main)
        with mock.patch.object(p.sys,'argv',['helper','--request','/unused','--perf-path','/bad','--private-directory','/private']):
            self.refused('request',p.main)

if __name__ == '__main__':
    unittest.main()
