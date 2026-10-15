"""Pure fixture/mocked lifecycle checks. These tests never invoke perf or sudo."""
import ast
import collections
import importlib.util
import io
import os
from pathlib import Path
import signal
import struct
import subprocess
import sys
import tempfile
import textwrap
import unittest
from unittest.mock import patch, Mock

sys.path.insert(0, str(Path(__file__).resolve().parent))
SPEC = importlib.util.spec_from_file_location("profile_preflight", Path(__file__).with_name("desktop-performance-profile.py"))
PROFILE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PROFILE)
TARGET = {"pid": 410, "ppid": 400, "start": 99, "uid": 1001, "tgid": 410}
TIDS = {410: TARGET}
INTERVAL = (1_000_000_000, 4_000_000_000)


def perf_file(types, sample_type=1 | 2 | 4 | 32 | 128 | 256, flags=(1 << 5) | (1 << 6)):
    data = bytearray(256)
    data[:8] = b"PERFILE2"
    struct.pack_into("<6Q", data, 8, 104, 128, 104, 128, 256, 8 * len(types))
    struct.pack_into("<II5Q", data, 104, 1, 112, 0, 1, sample_type, 0, flags)
    for kind in types:
        data.extend(struct.pack("<IHH", kind, 2, 8))
    return bytes(data)


def indexed_perf_file(entries=None, ids=(101, 102, 103, 104)):
    entries = entries if entries is not None else [(value, i, i, TARGET["pid"]) for i, value in enumerate(ids)]
    attr_offset = 104 + 8 * len(ids)
    data_offset = attr_offset + 128
    metadata = struct.pack("<IHHQ", 69, 0, 16 + 32 * len(entries), len(entries))
    metadata += b"".join(struct.pack("<QQQq", *entry) for entry in entries)
    records = metadata + b"".join(struct.pack("<IHH", kind, 2, 8) for kind in [3, 10] + [9] * 10)
    data = bytearray(data_offset)
    data[:8] = b"PERFILE2"
    struct.pack_into("<6Q", data, 8, 104, 128, attr_offset, 128, data_offset, len(records))
    struct.pack_into(f"<{len(ids)}Q", data, 104, *ids)
    struct.pack_into("<II5Q", data, attr_offset, 1, 112, 0, 1, 295, 0, (1 << 5) | (1 << 6))
    struct.pack_into("<QQ", data, attr_offset + 112, 104, len(ids) * 8)
    data.extend(records)
    return bytes(data)


def append_metadata(raw, records):
    data = bytearray(raw)
    start, size = struct.unpack_from("<QQ", data, 40)
    extra = b"".join(records)
    data.extend(extra)
    struct.pack_into("<Q", data, 48, size + len(extra))
    return bytes(data)


def metadata_record(kind, payload=b"", misc=0):
    return struct.pack("<IHH", kind, misc, 8 + len(payload)) + payload


def clock_update(subtype, payload, event_id=101):
    return metadata_record(78, struct.pack("<QQ", subtype, event_id) + payload)


def exported(samples=10):
    initial = " 410/ 410 0.000000000: PERF_RECORD_COMM: cpu-probe:410/410\n"
    initial += " 410/410 0.000000000: PERF_RECORD_MMAP2 410/410: [0x1(0x2) @ 0]: /owned/cpu-probe\n"
    for _ in range(samples):
        initial += (
            " 410/410 2.000000000: cpu-clock:u: 1 tl775_profile_probe_leaf (/owned/cpu-probe)\n"
            "        2 tl775_profile_probe_parent (/owned/cpu-probe)\n\n"
        )
    return initial


class EventScopeTests(unittest.TestCase):
    def test_typed_export_matches_complete_record_census_and_symbols(self):
        census = PROFILE.event_census(perf_file([3, 10] + [9] * 10))
        result = PROFILE.validate_export(exported(), census, TARGET, TIDS, INTERVAL)
        self.assertEqual(result["probe_chains"], 10)
        self.assertEqual(result["event_counts"], {"COMM": 1, "MMAP2": 1, "SAMPLE": 10})

    def test_unknown_sample_and_foreign_metadata_fail_entire_recording(self):
        for old, new in [("410/410 2.", "777/777 2."), ("MMAP2 410/410", "MMAP2 777/777")]:
            with self.subTest(new=new), self.assertRaises(PROFILE.Refused):
                PROFILE.validate_export(exported().replace(old, new), collections.Counter(COMM=1, MMAP2=1, SAMPLE=10), TARGET, TIDS, INTERVAL)

    def test_omitted_metadata_and_unrecognized_line_are_not_silently_ignored(self):
        census = collections.Counter(COMM=1, MMAP2=1, SAMPLE=10)
        for text in [exported().replace("PERF_RECORD_COMM", "PERF_RECORD_NAMESPACES"), exported() + "unknown\n"]:
            with self.assertRaises(PROFILE.Refused):
                PROFILE.validate_export(text, census, TARGET, TIDS, INTERVAL)
        with self.assertRaisesRegex(PROFILE.Refused, "census"):
            PROFILE.validate_export(exported().split("\n", 1)[1], census, TARGET, TIDS, INTERVAL)

    def test_outside_lifetime_and_new_unverified_thread_fail(self):
        census = collections.Counter(COMM=1, MMAP2=1, SAMPLE=10)
        for text in [exported().replace("2.000000000", "9.000000000"), exported().replace("410/410 2.", "410/411 2.")]:
            with self.assertRaises(PROFILE.Refused):
                PROFILE.validate_export(text, census, TARGET, TIDS, INTERVAL)

    def test_raw_stacks_registers_lost_events_unknown_events_and_inherit_refused(self):
        for bit in (1024, 4096, 8192):
            with self.assertRaises(PROFILE.Refused):
                PROFILE.event_census(perf_file([9], sample_type=2 | 4 | 32 | bit))
        for kind in (2, 13, 82, 999):
            with self.assertRaises(PROFILE.Refused):
                PROFILE.event_census(perf_file([kind]))
        with self.assertRaises(PROFILE.Refused):
            PROFILE.event_census(perf_file([9], flags=(1 << 1) | (1 << 5) | (1 << 6)))
        self.assertEqual(PROFILE.event_census(perf_file([9], flags=1 | (1 << 5) | (1 << 6))), {"SAMPLE": 1})

    def test_unknown_record_refusal_contains_only_safe_header_diagnostics(self):
        for kind in (2, 79, 999):
            with self.subTest(kind=kind), self.assertRaises(PROFILE.CensusRefused) as caught:
                PROFILE.event_census(perf_file([3, 9, kind]))
            error = caught.exception
            self.assertEqual(str(error), "unsupported_or_lost_perf_record")
            diagnostic = error.diagnostic.copy()
            inventory = diagnostic.pop("header_inventory")
            self.assertTrue(inventory["complete"])
            self.assertEqual(diagnostic, {
                "record_type": kind, "record_misc": 2, "record_length": 8,
                "preceding_record_counts": {"COMM": 1, "SAMPLE": 1},
                "attribute_sample_type": 1 | 2 | 4 | 32 | 128 | 256,
                "attribute_flags": (1 << 5) | (1 << 6),
            })

    def test_owned_id_index_is_verified_separately_from_typed_samples(self):
        census = PROFILE.event_census(indexed_perf_file(), TARGET["pid"], [0, 1, 2, 3])
        self.assertEqual(census, {"ID_INDEX": 1, "COMM": 1, "MMAP2": 1, "SAMPLE": 10})
        result = PROFILE.validate_export(exported(), census, TARGET, TIDS, INTERVAL)
        self.assertEqual(result["validated_header_records"], {"ID_INDEX": 1})
        self.assertEqual(result["probe_chains"], 10)
        singleton = indexed_perf_file([(101, 0, (1 << 64) - 1, TARGET["pid"])], ids=(101,))
        self.assertEqual(PROFILE.event_census(singleton, TARGET["pid"], [0, 1])["ID_INDEX"], 1)
        with self.assertRaisesRegex(PROFILE.Refused, "scope"):
            PROFILE.event_census(indexed_perf_file())

    def test_id_index_foreign_tid_wrong_id_ring_cpu_guest_and_truncation_refused(self):
        valid = [(101 + i, i, i, TARGET["pid"]) for i in range(4)]
        for field, value in ((0, 999), (1, 99), (2, 99), (3, 777), (3, -1)):
            entries = valid.copy()
            row = list(entries[0]); row[field] = value; entries[0] = tuple(row)
            with self.subTest(field=field, value=value), self.assertRaises(PROFILE.Refused):
                PROFILE.event_census(indexed_perf_file(entries), TARGET["pid"], [0, 1, 2, 3])
        raw = indexed_perf_file()
        offset = struct.unpack_from("<Q", raw, 40)[0]
        for mutate in ("truncated", "nr", "guest", "ids", "duplicate"):
            data = bytearray(raw)
            if mutate == "truncated":
                data = data[:-1]
            elif mutate == "nr":
                struct.pack_into("<Q", data, offset + 8, 5)
            elif mutate == "guest":
                struct.pack_into("<H", data, offset + 6, 16 + 48 * 4)
            elif mutate == "ids":
                struct.pack_into("<Q", data, 104, 999)
            else:
                struct.pack_into("<Q", data, 112, 101)
            with self.subTest(mutate=mutate), self.assertRaises(PROFILE.Refused):
                PROFILE.event_census(bytes(data), TARGET["pid"], [0, 1, 2, 3])

    def test_online_cpu_map_is_scoped_and_rejects_malformed_ranges(self):
        for text, expected in (("0-3", [0, 1, 2, 3]), ("0,2-3", [0, 2, 3])):
            with patch.object(PROFILE.Path, "read_text", return_value=text):
                self.assertEqual(PROFILE.online_cpu_map(), expected)
        for text in ("", "3-1", "0,0", "1,0", "0-999999", "unknown"):
            with patch.object(PROFILE.Path, "read_text", return_value=text), self.assertRaises(PROFILE.Refused):
                PROFILE.online_cpu_map()

    def test_clock_metadata_and_markers_keep_full_typed_sample_census(self):
        records = [clock_update(0, b"msec\0" + b"\0" * 19),
                   clock_update(1, struct.pack("<d", 1e-6) + b"\0" * 16),
                   metadata_record(73, struct.pack("<QQ", 1, TARGET["pid"]) + b"\0" * 16),
                   metadata_record(74, struct.pack("<HBBHH", 2, 0, 0, 0, 3)),
                   metadata_record(82), metadata_record(68), metadata_record(68)]
        census = PROFILE.event_census(append_metadata(indexed_perf_file(), records), TARGET["pid"], [0, 1, 2, 3])
        result = PROFILE.validate_export(exported(), census, TARGET, TIDS, INTERVAL)
        self.assertEqual(result["samples"], 10)
        self.assertEqual(result["validated_header_records"], {"ID_INDEX": 1, "EVENT_UPDATE_UNIT": 1,
            "EVENT_UPDATE_SCALE": 1, "THREAD_MAP": 1, "CPU_MAP": 1, "FINISHED_INIT": 1, "FINISHED_ROUND": 2})

    def test_known_metadata_foreign_ids_threads_unknown_subtypes_and_bad_values_refuse(self):
        bad = [clock_update(0, b"msec\0" + b"\0" * 19, 999),
               clock_update(0, b"other\0" + b"\0" * 18),
               clock_update(1, struct.pack("<d", float("nan")) + b"\0" * 16),
               clock_update(1, struct.pack("<d", 1.0) + b"\0" * 16),
               clock_update(2, b"cpu-clock:u\0"), clock_update(999, b""),
               metadata_record(73, struct.pack("<QQ", 1, 777) + b"\0" * 16),
               metadata_record(73, struct.pack("<QQ", 1, TARGET["pid"]) + b"foreign" + b"\0" * 9),
               metadata_record(74, struct.pack("<HBBHH", 2, 0, 0, 0, 4)),
               metadata_record(82, b"bad"), metadata_record(68, misc=1)]
        for record in bad:
            with self.subTest(record=record[:8]), self.assertRaises(PROFILE.Refused):
                PROFILE.event_census(append_metadata(indexed_perf_file(), [record]), TARGET["pid"], [0, 1, 2, 3])
        with self.assertRaisesRegex(PROFILE.Refused, "duplicate"):
            PROFILE.event_census(append_metadata(indexed_perf_file(), [metadata_record(82)] * 2), TARGET["pid"], [0, 1, 2, 3])

    def test_cpu_map_encodings_are_bounded_and_exact(self):
        valid = [struct.pack("<HH4H", 0, 4, 0, 1, 2, 3) + b"\0" * 4,
                 struct.pack("<HHHI", 1, 1, 4, 15) + b"\0" * 6,
                 struct.pack("<HHHIQ", 1, 1, 8, 0, 15) + b"\0" * 6,
                 struct.pack("<HBBHH", 2, 0, 0, 0, 3)]
        for payload in valid:
            self.assertEqual(PROFILE.decode_cpu_map(payload), [0, 1, 2, 3])
        for payload in (b"", b"\0" * 7, struct.pack("<HBBHH", 2, 0, 1, 0, 3),
                        valid[0][:-1] + b"x", struct.pack("<HHHH", 0, 2, 1, 1),
                        struct.pack("<HHHH", 1, 1, 3, 0)):
            with self.assertRaises(PROFILE.Refused):
                PROFILE.decode_cpu_map(payload)

    def test_unknown_stop_inventory_sees_all_headers_without_exporting_ids_or_payload(self):
        records = [clock_update(999, b"private-payload", 101), metadata_record(79, b""),
                   metadata_record(73, struct.pack("<QQ", 1, 777) + b"foreign-name" + b"\0" * 4),
                   clock_update(123456, b"secret", 987654321)]
        with self.assertRaises(PROFILE.CensusRefused) as caught:
            PROFILE.event_census(append_metadata(indexed_perf_file(), records), TARGET["pid"], [0, 1, 2, 3])
        inventory = caught.exception.diagnostic["header_inventory"]
        self.assertTrue(inventory["complete"])
        self.assertEqual({row["type"] for row in inventory["headers"]}, {69, 3, 10, 9, 78, 79, 73})
        self.assertIn({"event_id_matches": True, "subtype": 999, "count": 1}, inventory["event_updates"])
        self.assertIn({"event_id_matches": False, "subtype": None, "count": 1}, inventory["event_updates"])
        encoded = str(inventory)
        for private in ("987654321", "123456", "secret", "foreign-name", "private-payload"):
            self.assertNotIn(private, encoded)
        with self.assertRaisesRegex(PROFILE.Refused, "incomplete"):
            PROFILE.event_census(indexed_perf_file(), TARGET["pid"], [0, 1, 2, 3], deadline=0)

    def test_numeric_inventory_reports_explicit_capacity_limits(self):
        for kinds, expected in ((range(1000, 1130), "diagnostic_distinct_header_limit"),
                                ([999] * 131073, "diagnostic_limit_or_deadline")):
            raw = perf_file(kinds)
            offset, size = struct.unpack_from("<QQ", raw, 40)
            inventory = PROFILE.numeric_header_inventory(raw, offset, offset + size, 104, 128, None)
            self.assertFalse(inventory["complete"])
            self.assertEqual(inventory["incomplete_reason"], expected)
        updates = [clock_update(i, b"") for i in range(129)]
        raw = append_metadata(indexed_perf_file(), updates)
        attr_offset = struct.unpack_from("<Q", raw, 24)[0]
        offset, size = struct.unpack_from("<QQ", raw, 40)
        inventory = PROFILE.numeric_header_inventory(raw, offset, offset + size, attr_offset, 128, None)
        self.assertFalse(inventory["complete"])
        self.assertEqual(inventory["incomplete_reason"], "diagnostic_subtype_limit")

    def test_insufficient_symbolized_callchains_cannot_allow_build(self):
        with self.assertRaises(PROFILE.Refused):
            PROFILE.validate_export(exported(9), collections.Counter(COMM=1, MMAP2=1, SAMPLE=9), TARGET, TIDS, INTERVAL)
        with self.assertRaises(PROFILE.Refused):
            PROFILE.validate_export(exported().replace("tl775_profile_probe_parent", "unknown"), collections.Counter(COMM=1, MMAP2=1, SAMPLE=10), TARGET, TIDS, INTERVAL)


class PackagedPerfDiscoveryTests(unittest.TestCase):
    def canonical_packages(self):
        # Parse just the workflow's pure scoped owner function; never execute
        # workflow steps, package installation, perf, sudo or any probe.
        workflow = Path(__file__).resolve().parents[1] / ".github/workflows/release-cpu-profile-preflight.yml"
        text = workflow.read_text()
        start = text.index("          def canonical_packages(")
        end = text.index("          def discover():", start)
        module = ast.parse(textwrap.dedent(text[start:end]))
        namespace = {}
        exec(compile(module, str(workflow), "exec"), namespace)
        return namespace["canonical_packages"]

    def test_canonical_vendor_package_requires_installed_exact_file_ownership(self):
        canonical = "/usr/lib/linux-azure-6.8-tools-6.8.0-1064/perf"
        package = "linux-azure-6.8-tools-6.8.0-1064"
        for installed, member in ((True, True), (False, True), (True, False)):
            calls = []
            def query(args):
                calls.append(args)
                if args[1] == "-S":
                    return Mock(returncode=0, stdout=f"{package}: {canonical}\n")
                if args[1] == "-W":
                    return Mock(returncode=0, stdout="ii \n" if installed else "rc \n")
                return Mock(returncode=0, stdout=canonical + "\n" if member else "/another/perf\n")
            checks = []
            actual = self.canonical_packages()(canonical, {}, query, checks)
            self.assertEqual(actual, [package] if installed and member else [])
            self.assertEqual(calls, [
                ["dpkg-query", "-S", canonical],
                ["dpkg-query", "-W", "-f=${db:Status-Abbrev}\n", package],
                ["dpkg-query", "-L", package],
            ])
            self.assertEqual(checks[0]["owners"][0]["installed"], installed)
            self.assertEqual(checks[0]["owners"][0]["exact_file_list"], member)

    def test_unrelated_owner_line_and_missing_owner_are_not_accepted(self):
        for result in (Mock(returncode=1, stdout=""), Mock(returncode=0, stdout="vendor: /different/perf\n")):
            query = Mock(return_value=result)
            self.assertEqual(self.canonical_packages()("/scoped/perf", {}, query, []), [])
            query.assert_called_once_with(["dpkg-query", "-S", "/scoped/perf"])


class OwnershipTests(unittest.TestCase):
    def test_root_uid_is_derived_from_sudo_and_coordinator_ancestry(self):
        coordinator = {**TARGET, "pid": 400, "ppid": 1, "tgid": 400}
        with patch.object(PROFILE.os, "getuid", return_value=0), patch.object(PROFILE.os, "getppid", return_value=400), patch.object(PROFILE, "same_process", return_value=True), patch.object(PROFILE, "process_identity", return_value=coordinator), patch.dict(os.environ, {"SUDO_UID": "1001", "SUDO_GID": "1001"}):
            self.assertEqual(PROFILE.derived_uid({"coordinator": coordinator}), 1001)
            with patch.dict(os.environ, {"SUDO_UID": "999"}), self.assertRaises(PROFILE.Refused):
                PROFILE.derived_uid({"coordinator": coordinator})

    def test_wrong_target_and_unrelated_same_uid_process_refused(self):
        request = {"coordinator": {"pid": 400}, "target": TARGET}
        with patch.object(PROFILE, "same_process", return_value=False), self.assertRaises(PROFILE.Refused):
            PROFILE.verify_probe(request, 1001)
        with patch.object(PROFILE, "same_process", return_value=True):
            with self.assertRaises(PROFILE.Refused):
                PROFILE.verify_probe(request, 999)
            request["target"] = {**TARGET, "ppid": 399}
            with self.assertRaises(PROFILE.Refused):
                PROFILE.verify_probe(request, 1001)

    def test_root_output_is_private_anchored_and_symlinks_are_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            anchor, fd = PROFILE.private_spool(directory, "owned")
            try:
                PROFILE.write_receipt(fd, {"nonce": "fixed"})
                os.rename(Path(directory) / "owned", Path(directory) / "moved")
                with self.assertRaises((PROFILE.Refused, FileNotFoundError)):
                    PROFILE.clean_spool(anchor, fd, "owned")
            finally:
                os.close(fd)
                os.close(anchor)
            link = Path(directory) / "link"
            link.symlink_to(Path(directory) / "moved")
            with self.assertRaises(OSError):
                PROFILE.open_directory(link)

    def test_cleanup_signals_only_saved_pidfd_and_reaps_child(self):
        child = Mock()
        child.poll.return_value = None
        child.wait.side_effect = [subprocess.TimeoutExpired("perf", 3), 0]
        with patch.object(PROFILE.signal, "pidfd_send_signal", create=True) as send:
            self.assertEqual(PROFILE.stop_owned(child, 91), 0)
            self.assertEqual(send.call_args_list[0].args, (91, signal.SIGINT))
            self.assertEqual(send.call_args_list[1].args, (91, signal.SIGTERM))
            self.assertEqual(send.call_count, 2)

    def test_pidfd_acquisition_failure_can_still_stop_exact_unreaped_popen_child(self):
        child = Mock()
        child.poll.return_value = None
        child.wait.side_effect = [subprocess.TimeoutExpired("perf", 3), subprocess.TimeoutExpired("perf", 2), 0]
        with patch.object(PROFILE.signal, "pidfd_send_signal", create=True) as send:
            self.assertEqual(PROFILE.stop_owned(child, None), 0)
            self.assertEqual([call.args[0] for call in child.send_signal.call_args_list], [signal.SIGINT, signal.SIGTERM, signal.SIGKILL])
            send.assert_not_called()

    def test_absolute_deadline_never_adds_another_full_wait(self):
        with patch.object(PROFILE.time, "monotonic", return_value=49.5):
            self.assertEqual(PROFILE.bounded_timeout(50, 6), .5)
        child = Mock()
        child.poll.return_value = None
        with patch.object(PROFILE.time, "monotonic", return_value=50), self.assertRaisesRegex(PROFILE.Refused, "no_exit_proof"):
            PROFILE.stop_owned(child, None, 50)
        child.kill.assert_called_once()
        child.wait.assert_not_called()

    def test_fixed_argv_and_config_do_not_allow_host_recording_or_home_changes(self):
        command = PROFILE.perf_command("/owned/perf", 410)
        self.assertEqual(command[command.index("--pid") + 1], "410")
        self.assertIn("--no-inherit", command)
        self.assertIn("--no-buildid-cache", command)
        self.assertIn("--no-bpf-event", command)
        self.assertIn("--synth=task,mmap", command)
        self.assertNotIn("--synth=all", command)
        self.assertNotIn("-a", command)
        self.assertNotIn("dwarf", command)
        self.assertNotIn("HOME", PROFILE.isolated_env())
        self.assertEqual(PROFILE.isolated_env()["DEBUGINFOD_URLS"], "")

    def test_perf_config_uses_supported_list_with_isolated_environment(self):
        result = Mock(returncode=0, stdout=b"")
        with patch.object(PROFILE.subprocess, "run", return_value=result) as run:
            PROFILE.verify_perf_config("/verified/perf", 91, PROFILE.time.monotonic() + 1)
        self.assertEqual(run.call_args.args[0], ["/verified/perf", "config", "--list"])
        self.assertNotIn("--show-origin", run.call_args.args[0])
        environment = run.call_args.kwargs["env"]
        self.assertEqual(environment["PERF_CONFIG_NOGLOBAL"], "1")
        self.assertEqual(environment["PERF_CONFIG_NOSYSTEM"], "1")
        self.assertNotIn("HOME", environment)
        with patch.object(PROFILE.subprocess, "run", return_value=Mock(returncode=0, stdout=b"buildid.dir=/outside\n")), self.assertRaisesRegex(PROFILE.Refused, "configuration_not_isolated"):
            PROFILE.verify_perf_config("/verified/perf", 91, PROFILE.time.monotonic() + 1)


class PreflightPolicyTests(unittest.TestCase):
    def run_preflight(self, directory, outcomes, expected_error=None):
        perf = Path(directory) / "perf"
        perf.write_bytes(b"synthetic test fixture, never executed")
        probe = Mock()
        probe.stdout = io.StringIO("READY\n")
        probe.poll.return_value = None
        probe.wait.return_value = 0
        coordinator = {**TARGET, "pid": 400, "sha256": "fixture-executable-hash"}
        root = Path(directory)
        with patch.object(PROFILE.sys, "platform", "linux"), patch.object(PROFILE.os, "getuid", return_value=1001), patch.object(PROFILE, "find_perf", return_value=str(perf)), patch.object(PROFILE.shutil, "which", return_value="/never/executed/cc"), patch.object(PROFILE, "artifact_path", return_value=root), patch.object(PROFILE, "process_identity", return_value=coordinator), patch.object(PROFILE, "build_id", return_value="abc123"), patch.object(PROFILE.subprocess, "run", return_value=Mock(stdout="perf fixture version")), patch.object(PROFILE.subprocess, "Popen", return_value=probe), patch.object(PROFILE.select, "select", return_value=([probe.stdout], [], [])), patch.object(PROFILE.os, "pidfd_open", create=True), patch.object(PROFILE.signal, "pidfd_send_signal", create=True), patch.object(PROFILE, "collect_attempt", side_effect=outcomes) as attempts:
            if expected_error is None:
                status = PROFILE.preflight(root / "receipt.json")
            else:
                with self.assertRaises(expected_error):
                    PROFILE.preflight(root / "receipt.json")
                status = None
        if expected_error is not None:
            probe.send_signal.assert_not_called()
            probe.wait.assert_not_called()
            self.assertTrue(list(root.glob("tl775-probe-source-*")))
            return status, [call.args[1] for call in attempts.call_args_list]
        self.assertTrue(probe.send_signal.called)
        self.assertTrue(probe.wait.called)
        self.assertFalse(list(root.glob("tl775-probe-source-*")))
        return status, [call.args[1] for call in attempts.call_args_list]

    def test_only_permission_denial_allows_exactly_one_privileged_probe(self):
        with tempfile.TemporaryDirectory() as directory:
            result, attempts = self.run_preflight(directory, [
                {"status": "STOP", "code": "perf_permission_denied"},
                {"status": "CAPABLE", "recorder_gone": True},
            ])
            self.assertEqual((result, attempts), (0, [False, True]))

    def test_missing_symbols_do_not_escalate_or_allow_canonical_build(self):
        with tempfile.TemporaryDirectory() as directory:
            result, attempts = self.run_preflight(directory, [
                {"status": "STOP", "code": "symbolized_probe_callchain_unavailable"},
            ])
            self.assertEqual((result, attempts), (2, [False]))
            import json
            self.assertFalse(json.loads((Path(directory) / "receipt.json").read_text())["canonical_build_allowed"])

    def test_missing_perf_stops_before_probe_compile_or_any_process(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(PROFILE.sys, "platform", "linux"), patch.object(PROFILE.os, "getuid", return_value=1001), patch.object(PROFILE, "artifact_path", return_value=Path(directory)), patch.object(PROFILE.os, "pidfd_open", create=True), patch.object(PROFILE.signal, "pidfd_send_signal", create=True), patch.object(PROFILE, "find_perf", side_effect=PROFILE.Refused("perf_direct_elf_unavailable")), patch.object(PROFILE.subprocess, "run") as run, patch.object(PROFILE.subprocess, "Popen") as spawn:
            with self.assertRaisesRegex(PROFILE.Refused, "perf_direct_elf_unavailable"):
                PROFILE.preflight(Path(directory) / "receipt.json")
            run.assert_not_called()
            spawn.assert_not_called()

    def test_malformed_terminal_or_broken_pipe_cannot_stop_target_before_recorder_proof(self):
        for error in [ValueError("malformed terminal"), BrokenPipeError("helper pipe closed"), PROFILE.Refused("helper_cleanup_unverified")]:
            with self.subTest(error=type(error).__name__), tempfile.TemporaryDirectory() as directory:
                _, attempts = self.run_preflight(directory, [error], type(error))
                self.assertEqual(attempts, [False])


if __name__ == "__main__":
    unittest.main()
