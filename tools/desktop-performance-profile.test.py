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

    def test_owned_synthetic_zero_prefix_metadata_and_verified_parent_fork(self):
        text = exported().replace("410/ 410 0.", "0/0 0.").replace("410/410 0.", "0/0 0.")
        text = " 0/0 0.000000000: PERF_RECORD_FORK(410:410):(400:400)\n" + text
        census = collections.Counter(COMM=1, MMAP2=1, FORK=1, SAMPLE=10)
        self.assertEqual(PROFILE.validate_export(text, census, TARGET, TIDS, INTERVAL)["probe_chains"], 10)

    def test_synthetic_prefix_never_excuses_foreign_samples_payloads_or_unverified_parent(self):
        valid = exported().replace("410/ 410 0.", "0/0 0.").replace("410/410 0.", "0/0 0.")
        bad = [valid.replace("410/410 2.", "0/0 2."),
               valid.replace("0/0 0.", "777/777 0."),
               valid.replace("0/0 0.", "0/0 2."),
               valid.replace("MMAP2 410/410", "MMAP2 777/777"),
               "0/0 0.000000000: PERF_RECORD_FORK(410:410):(399:399)\n" + valid]
        for text in bad:
            with self.assertRaises(PROFILE.Refused):
                PROFILE.validate_export(text, collections.Counter(COMM=1, MMAP2=1, SAMPLE=10), TARGET, TIDS, INTERVAL)

    def test_typed_stop_diagnostics_never_export_raw_identities_or_text(self):
        text = "0/0 0.000000000: PERF_RECORD_FORK(410:410):(987654:987654) private-path\n"
        with self.assertRaises(PROFILE.TypedRefused) as caught:
            PROFILE.validate_export(text, collections.Counter(FORK=1), TARGET, TIDS, INTERVAL)
        self.assertEqual(caught.exception.diagnostic, {"event_kind": "FORK", "prefix_class": "zero_placeholder",
            "payload_owner_matches": False, "time_class": "zero"})
        for raw in ("987654", "410", "private-path"):
            self.assertNotIn(raw, str(caught.exception.diagnostic))
        sample = "0/0 2.000000000: cpu-clock:u: 1 foreign-symbol (/foreign/path)\n"
        with self.assertRaises(PROFILE.TypedRefused) as sample_error:
            PROFILE.validate_export(sample, collections.Counter(SAMPLE=1), TARGET, TIDS, INTERVAL)
        self.assertEqual(sample_error.exception.diagnostic, {"event_kind": "SAMPLE", "prefix_class": "zero_placeholder",
            "payload_owner_matches": None, "time_class": "within_verified_lifetime"})

    def test_callchain_stop_exposes_only_known_probe_numeric_counts(self):
        text = exported().replace("tl775_profile_probe_parent", "foreign-secret-symbol")
        with self.assertRaises(PROFILE.Refused) as caught:
            PROFILE.validate_export(text, collections.Counter(COMM=1, MMAP2=1, SAMPLE=10), TARGET, TIDS, INTERVAL)
        self.assertEqual(getattr(caught.exception, "diagnostic", None), {
            "sampled": 10, "leaf_present": 10, "parent_present": 0, "dual_present": 0})
        self.assertNotIn("foreign-secret-symbol", str(getattr(caught.exception, "diagnostic", None)))
        with self.assertRaises(PROFILE.Refused) as low:
            PROFILE.validate_export(exported(9), collections.Counter(COMM=1, MMAP2=1, SAMPLE=9), TARGET, TIDS, INTERVAL)
        self.assertEqual(low.exception.diagnostic, {"sampled": 9, "leaf_present": 9, "parent_present": 9, "dual_present": 9})

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

    def test_tool_query_timeout_uses_ten_seconds_with_remaining_global_budget(self):
        workflow = Path(__file__).resolve().parents[1] / ".github/workflows/release-cpu-profile-preflight.yml"
        text = workflow.read_text()
        start = text.index("          def query(args):")
        end = text.index("          def canonical_packages(", start)
        module = ast.parse(textwrap.dedent(text[start:end]))
        for remaining in (50, 5, 0):
            tool = Mock()
            namespace = {"time": Mock(monotonic=Mock(return_value=100 - remaining)),
                         "deadline": 100, "subprocess": tool}
            exec(compile(module, str(workflow), "exec"), namespace)
            if not remaining:
                with self.assertRaises(TimeoutError):
                    namespace["query"](["dpkg-query", "-S", "/exact/perf"])
                tool.run.assert_not_called()
            else:
                namespace["query"](["dpkg-query", "-S", "/exact/perf"])
                tool.run.assert_called_once_with(["dpkg-query", "-S", "/exact/perf"],
                    text=True, capture_output=True, timeout=min(10, remaining))

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

    def test_expected_probe_symbols_require_actual_defined_global_functions(self):
        lines = [f"17: 0000000000001234 48 FUNC GLOBAL DEFAULT 15 {name}" for name in PROFILE.PROBE_NAMES]
        with patch.object(PROFILE.subprocess, "run", return_value=Mock(stdout="\n".join(lines))) as run:
            self.assertTrue(PROFILE.verify_probe_elf_symbols("/owned/cpu-probe", PROFILE.time.monotonic() + 2))
        self.assertEqual(run.call_args.args[0], ["readelf", "--symbols", "--wide", "/owned/cpu-probe"])
        self.assertNotIn("HOME", run.call_args.kwargs["env"])
        for text in (lines[0], "\n".join(lines).replace("FUNC", "OBJECT"),
                     "\n".join(lines).replace("DEFAULT 15", "DEFAULT UND")):
            with patch.object(PROFILE.subprocess, "run", return_value=Mock(stdout=text)), self.assertRaises(PROFILE.Refused):
                PROFILE.verify_probe_elf_symbols("/owned/cpu-probe", PROFILE.time.monotonic() + 2)

    def test_compiled_probe_frame_and_callproof_is_fixed_x86_64_and_private(self):
        header = "Class: ELF64\nData: 2's complement, little endian\nMachine: Advanced Micro Devices X86-64\n"
        parent, leaf = PROFILE.PROBE_NAMES
        def body(name, frame=True, call=False):
            entry = "1000: push rbp\n1001: mov rbp,rsp\n" if frame else "1000: mov eax,0x186a0\n"
            return f"/private/own-elf: fileformat\n1000 <{name}>:\n" + entry + (f"1004: call 2000 <{leaf}>\n" if call else "") + "1009: ret\n"
        with patch.object(PROFILE.subprocess, "run", side_effect=[Mock(stdout=header), Mock(stdout=body(parent, call=True)), Mock(stdout=body(leaf))]) as run:
            proof = PROFILE.verify_probe_machine_frame("/private/own-elf", PROFILE.time.monotonic() + 10)
        self.assertTrue(all(proof.values()))
        self.assertEqual(run.call_args_list[1].args[0], ["objdump", "--disassemble=" + parent, "--no-show-raw-insn", "-M", "intel", "/private/own-elf"])
        self.assertEqual(run.call_args_list[2].args[0][1], "--disassemble=" + leaf)
        with patch.object(PROFILE.subprocess, "run", side_effect=[Mock(stdout=header), Mock(stdout=body(parent, call=True)), Mock(stdout=body(leaf, frame=False))]), self.assertRaises(PROFILE.MachineFrameRefused) as missing:
            PROFILE.verify_probe_machine_frame("/private/own-elf", PROFILE.time.monotonic() + 10)
        self.assertTrue(missing.exception.diagnostic["leaf_body_decoded"])
        self.assertFalse(missing.exception.diagnostic["leaf_entry_frame_verified"])
        self.assertTrue(missing.exception.diagnostic["parent_calls_leaf_verified"])
        self.assertNotIn("1000", str(missing.exception.diagnostic))
        self.assertNotIn("private", str(missing.exception.diagnostic))
        for parent_body in (body(parent), body(parent, call=True).replace("<" + leaf + ">", "<foreign-function>")):
            with patch.object(PROFILE.subprocess, "run", side_effect=[Mock(stdout=header), Mock(stdout=parent_body), Mock(stdout=body(leaf))]), self.assertRaises(PROFILE.MachineFrameRefused) as wrong_call:
                PROFILE.verify_probe_machine_frame("/private/own-elf", PROFILE.time.monotonic() + 10)
            self.assertFalse(wrong_call.exception.diagnostic["parent_calls_leaf_verified"])
        with patch.object(PROFILE.subprocess, "run", side_effect=FileNotFoundError("objdump unavailable")), self.assertRaises(PROFILE.MachineFrameRefused) as tool:
            PROFILE.verify_probe_machine_frame("/private/own-elf", PROFILE.time.monotonic() + 10)
        self.assertFalse(tool.exception.diagnostic["verified"])
        with patch.object(PROFILE.subprocess, "run", return_value=Mock(stdout=header.replace("ELF64", "ELF32"))) as wrong, self.assertRaises(PROFILE.MachineFrameRefused):
            PROFILE.verify_probe_machine_frame("/private/own-elf", PROFILE.time.monotonic() + 10)
        self.assertEqual(wrong.call_count, 1)
        self.assertFalse(PROFILE.entry_frame_verified([("push", "rbp"), ("mov", "rsp,rbp")]))
        self.assertEqual(PROFILE.probe_instructions(body("foreign"), leaf), [])

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
        with patch.object(PROFILE.sys, "platform", "linux"), patch.object(PROFILE.os, "getuid", return_value=1001), patch.object(PROFILE, "find_perf", return_value=str(perf)), patch.object(PROFILE.shutil, "which", return_value="/never/executed/cc"), patch.object(PROFILE, "artifact_path", return_value=root), patch.object(PROFILE, "process_identity", return_value=coordinator), patch.object(PROFILE, "build_id", return_value="abc123"), patch.object(PROFILE, "verify_probe_elf_symbols", return_value=True), patch.object(PROFILE, "verify_probe_machine_frame", return_value={"verified": True}), patch.object(PROFILE.subprocess, "run", return_value=Mock(stdout="perf fixture version")), patch.object(PROFILE.subprocess, "Popen", return_value=probe), patch.object(PROFILE.select, "select", return_value=([probe.stdout], [], [])), patch.object(PROFILE.os, "pidfd_open", create=True), patch.object(PROFILE.signal, "pidfd_send_signal", create=True), patch.object(PROFILE, "collect_attempt", side_effect=outcomes) as attempts:
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

    def test_missing_expected_elf_symbols_stops_before_target_or_recorder(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(PROFILE.sys, "platform", "linux"), patch.object(PROFILE.os, "getuid", return_value=1001), patch.object(PROFILE, "artifact_path", return_value=Path(directory)), patch.object(PROFILE.os, "pidfd_open", create=True), patch.object(PROFILE.signal, "pidfd_send_signal", create=True), patch.object(PROFILE, "find_perf", return_value="/owned/perf"), patch.object(PROFILE.shutil, "which", return_value="/never/executed/cc"), patch.object(PROFILE.subprocess, "run"), patch.object(PROFILE, "verify_probe_elf_symbols", side_effect=PROFILE.Refused("expected_probe_elf_symbols_unverified")), patch.object(PROFILE.subprocess, "Popen") as spawn, patch.object(PROFILE, "collect_attempt") as collect:
            with self.assertRaisesRegex(PROFILE.Refused, "expected_probe_elf_symbols_unverified"):
                PROFILE.preflight(Path(directory) / "receipt.json")
            spawn.assert_not_called()
            collect.assert_not_called()
            self.assertFalse(list(Path(directory).glob("tl775-probe-source-*")))

    def test_unverified_machine_frame_stops_before_target_or_recorder(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(PROFILE.sys, "platform", "linux"), patch.object(PROFILE.os, "getuid", return_value=1001), patch.object(PROFILE, "artifact_path", return_value=Path(directory)), patch.object(PROFILE.os, "pidfd_open", create=True), patch.object(PROFILE.signal, "pidfd_send_signal", create=True), patch.object(PROFILE, "find_perf", return_value="/owned/perf"), patch.object(PROFILE.shutil, "which", return_value="/never/executed/cc"), patch.object(PROFILE.subprocess, "run", return_value=Mock(stdout="fixture version")), patch.object(PROFILE, "verify_probe_elf_symbols", return_value=True), patch.object(PROFILE, "verify_probe_machine_frame", side_effect=PROFILE.MachineFrameRefused({"x86_64_elf_verified": True, "leaf_body_decoded": True, "parent_body_decoded": True, "leaf_entry_frame_verified": False, "parent_entry_frame_verified": False, "parent_calls_leaf_verified": True, "verified": False})), patch.object(PROFILE.subprocess, "Popen") as spawn, patch.object(PROFILE, "collect_attempt", return_value={"status": "STOP", "recorder_gone": True}), patch.object(PROFILE, "process_identity", return_value={**TARGET, "sha256": "fixture"}), patch.object(PROFILE, "build_id", return_value="fixture"), patch.object(PROFILE.select, "select", return_value=([], [], [])):
            with self.assertRaisesRegex(PROFILE.Refused, "expected_probe_machine_frame_unverified"):
                PROFILE.preflight(Path(directory) / "receipt.json")
            spawn.assert_not_called()
            self.assertFalse(list(Path(directory).glob("tl775-probe-source-*")))

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



class OwnedApplicationTests(unittest.TestCase):
    def manifest(self):
        return {"schema": 1, "source_sha": "a" * 40, "coordinator_pid": 400,
                "launch_pid": 410, "measurement_started_at_utc": "2026-10-16T00:00:00Z",
                "measurement_deadline_utc": "2026-10-16T00:01:00Z",
                "roles": [{"role": role, "sha256": "b" * 64, "build_id": "c" * 40}
                          for role in ("server", "native_renderer")],
                "case_id": "sixteen_universe", "execution_mode": "unrestricted"}

    def test_only_exact_existing_case_and_role_pins(self):
        PROFILE.validate_app_manifest(self.manifest())
        for key, replacement in [("case_id", "demo"), ("execution_mode", "single_core"),
                                 ("source_sha", "main"), ("measurement_deadline_utc", "2026-10-16T00:02:00Z")]:
            value = self.manifest()
            value[key] = replacement
            with self.subTest(key=key), self.assertRaises(PROFILE.Refused):
                PROFILE.validate_app_manifest(value)
        value = self.manifest()
        value['unreviewed'] = True
        with self.assertRaises(PROFILE.Refused):
            PROFILE.validate_app_manifest(value)
        value = self.manifest()
        value['roles'][1]['role'] = 'server'
        with self.assertRaises(PROFILE.Refused):
            PROFILE.validate_app_manifest(value)

    def test_own_elf_pie_ip_mapping_ignores_display_symbol_and_dso(self):
        resolve = PROFILE.OwnElfResolver([(0x70001000, 0x70002000, 0x1000)],
                                        [(0x1000, 0x1000, 0x1000, True)],
                                        [(0x1100, 0x80, 'actual_owned_function')])
        samples = [['cpu-clock:u: 70001110 invented_secret (/untrusted/path)',
                    '70001120 foreign_name (/foreign/path)'] for _ in range(10)]
        result = PROFILE.aggregate_owned_samples(samples, collections.Counter(SAMPLE=10), {}, resolve)
        self.assertEqual(result['own_elf_samples'], 10)
        self.assertEqual(result['leaf_top20'], [{'function': 'actual_owned_function', 'samples': 10}])
        self.assertNotIn('secret', str(result))
        self.assertNotIn('/foreign', str(result))
        self.assertEqual(resolve(0x70001900), '[own ELF unresolved]')
        self.assertEqual(resolve(0x80000000), '[external or unmapped]')

    def test_non_pie_and_partial_external_callers_are_truthful(self):
        resolve = PROFILE.OwnElfResolver([(0x401000, 0x402000, 0x1000)],
                                        [(0x1000, 0x401000, 0x1000, True)],
                                        [(0x401100, 0x80, 'owned')])
        self.assertEqual(resolve(0x401110), 'owned')
        samples = [['cpu-clock:u: 401110 ignored (ignored)'] for _ in range(10)]
        result = PROFILE.aggregate_owned_samples(samples, collections.Counter(SAMPLE=10), {}, resolve)
        self.assertEqual(result['samples_with_callers'], 0)
        self.assertEqual(result['caller_coverage'], 'partial_frame_pointer_only')
        with self.assertRaises(PROFILE.Refused):
            PROFILE.aggregate_owned_samples([['cpu-clock:u: 80000000 ignored (ignored)']] * 10, {}, {}, resolve)
        with self.assertRaises(PROFILE.Refused):
            PROFILE.aggregate_owned_samples(samples[:9], {}, {}, resolve)

    def test_multithread_per_thread_id_index_exact_scope(self):
        entries = [(101, 0, (1 << 64) - 1, 410), (102, 1, (1 << 64) - 1, 411)]
        data = indexed_perf_file(entries, ids=(101, 102))
        PROFILE.event_census(data, [410, 411], [0, 1])
        for invalid in [[(101, 0, (1 << 64) - 1, 410), (102, 1, (1 << 64) - 1, 999)],
                        [(101, 1, (1 << 64) - 1, 410), (102, 0, (1 << 64) - 1, 411)],
                        [(101, 0, (1 << 64) - 1, 410), (102, 1, (1 << 64) - 1, 410)]]:
            with self.subTest(entries=invalid), self.assertRaises(PROFILE.Refused):
                PROFILE.event_census(indexed_perf_file(invalid, ids=(101, 102)), [410, 411], [0, 1])

    def test_multithread_cpu_matrix_requires_every_pair_once(self):
        entries = [(101, 0, 0, 410), (102, 0, 0, 411),
                   (103, 1, 1, 410), (104, 1, 1, 411)]
        PROFILE.event_census(indexed_perf_file(entries), [410, 411], [0, 1])
        entries[-1] = (104, 1, 1, 999)
        with self.assertRaises(PROFILE.Refused):
            PROFILE.event_census(indexed_perf_file(entries), [410, 411], [0, 1])

    def test_thread_map_exact_owned_set_no_comm_export(self):
        record = metadata_record(73, struct.pack('<Q', 2) +
                                 struct.pack('<Q16s', 410, b'private-name') +
                                 struct.pack('<Q16s', 411, b'another-name'))
        self.assertEqual(PROFILE.validate_metadata(73, record, (101, 102), [410, 411], [0, 1]), 'THREAD_MAP')
        with self.assertRaises(PROFILE.Refused):
            PROFILE.validate_metadata(73, record, (101, 102), [410, 999], [0, 1])

    def test_application_thread_lifecycle_payloads_never_authorize_foreign_samples(self):
        resolve = PROFILE.OwnElfResolver([(0x1000, 0x2000, 0x1000)],
                                        [(0x1000, 0x1000, 0x1000, True)], [(0x1100, 128, 'owned')])
        tids = {410: TARGET, 411: {**TARGET, 'pid': 411}}
        text = ('0/0 0.000000000: PERF_RECORD_FORK (410:411):(410:410)\n'
                '0/0 0.000000000: PERF_RECORD_COMM private-thread:410/411\n' +
                '410/411 2.000000000: cpu-clock:u: 1110 untrusted (untrusted)\n\n' * 10)
        census = collections.Counter(FORK=1, COMM=1, SAMPLE=10)
        result = PROFILE.validate_export(text, census, TARGET, tids, INTERVAL, resolve)
        self.assertEqual(result['samples'], 10)
        for changed in [text.replace('410/411 2.', '999/411 2.'),
                        text.replace('(410:411):(410:410)', '(999:411):(410:410)'),
                        text.replace('private-thread:410/411', 'private-thread:999/411'),
                        text.replace('410/411 2.', '410/411 9.')]:
            with self.subTest(text=changed[:30]), self.assertRaises(PROFILE.Refused):
                PROFILE.validate_export(changed, census, TARGET, tids, INTERVAL, resolve)
        self.assertNotIn('private-thread', str(result))
        self.assertNotIn('untrusted', str(result))

    def test_role_sha_mismatch_stop_has_numeric_diagnostic_and_starts_no_recorder(self):
        import datetime
        import json
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            value = self.manifest()
            now = datetime.datetime.now(datetime.timezone.utc)
            value.update(coordinator_pid=400, launch_pid=410,
                         measurement_started_at_utc=now.isoformat(),
                         measurement_deadline_utc=(now + datetime.timedelta(seconds=60)).isoformat())
            request, output = root / 'request.json', root / 'receipt.json'
            request.write_text(json.dumps(value))
            identities = {pid: {**TARGET, 'pid': pid, 'tgid': pid, 'ppid': 400, 'sha256': 'd' * 64,
                                'exe': '/private/owned-binary', 'inode': [1, 2, 3, 4]}
                          for pid in (400, 410, 420, 421)}
            original_resolve = Path.resolve
            def resolve(path, *args, **kwargs):
                if str(path) == '/proc/420/exe':
                    return Path('/private/light-headless')
                if str(path) == '/proc/421/exe':
                    return Path('/private/viz-renderer')
                return original_resolve(path, *args, **kwargs)
            with patch.object(PROFILE, 'artifact_path', return_value=root), patch.object(PROFILE.sys, 'platform', 'linux'), patch.object(PROFILE.os, 'getuid', return_value=1001), patch.object(PROFILE.os, 'getppid', return_value=400), patch.object(PROFILE, 'pinned_process', side_effect=lambda pid: identities.get(pid, {**identities[400], 'pid': pid, 'tgid': pid})), patch.object(PROFILE, 'owned_descendants', return_value={420: identities[420], 421: identities[421]}), patch.object(PROFILE, 'find_perf', return_value='/private/perf'), patch.object(PROFILE, 'digest', return_value='f' * 64), patch.object(PROFILE, 'build_id', return_value='c' * 40), patch.object(Path, 'resolve', resolve), patch.object(PROFILE, 'collect_attempt') as collect:
                self.assertEqual(PROFILE.owned_app(request, output), 2)
            report = json.loads(output.read_text())
            self.assertEqual(report['code'], 'exact_owned_application_role_not_unique')
            self.assertTrue(report['recorder_cleanup_verified'])
            self.assertEqual(report.get('role_binding_diagnostic'), [{'role': 'server',
                'owned_name_candidates': 1, 'sha256_matches': 0, 'build_id_matches': 1, 'exact_matches': 0},
                {'role': 'native_renderer', 'owned_name_candidates': 1, 'sha256_matches': 0,
                 'build_id_matches': 1, 'exact_matches': 0}])
            collect.assert_not_called()
            for private in ['420', '/private', 'owned-binary']:
                self.assertNotIn(private, json.dumps(report))

    def test_workflow_descriptor_comes_from_verified_actual_packaged_sidecars(self):
        import importlib.util
        import json
        import types
        workflow = Path(__file__).resolve().parents[1] / '.github/workflows/release-owned-app-cpu-profile.yml'
        lines = workflow.read_text().splitlines()
        start = next(i for i, line in enumerate(lines) if line.strip() == 'import hashlib, importlib.util, json, os, pathlib, sys, time')
        end = next(i for i in range(start, len(lines)) if lines[i].strip() == 'PYTHON')
        code = '\n'.join(line[10:] for line in lines[start:end])
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            candidate, output, temporary = root / 'candidate', root / 'out', root / 'tmp'
            for folder in (candidate, output, temporary):
                folder.mkdir()
            release = candidate / '.artifacts/build/cargo/x86_64-unknown-linux-gnu/release'
            app = release / 'bundle/appimage/Control.AppImage'
            app.parent.mkdir(parents=True)
            app.write_bytes(b'exact image')
            for name in ('light-headless', 'viz-renderer'):
                (release / name).write_bytes(b'cargo output ' + name.encode())
            for name in ('Cargo.lock', 'package-lock.json'):
                (candidate / name).write_text('fixed')
            (output / 'application-source-binaries.json').write_text('{}')
            roles = [{'role': role, 'sha256': '9' * 64, 'build_id': '8' * 40}
                     for role in ('server', 'native_renderer')]
            proof = {'roles': roles, 'appimage_sha256': PROFILE.digest(app),
                     'source_verified': True, 'fixed_packaged_paths_verified': True, 'comparisons': []}
            package_bind = Mock(return_value=proof)
            fake = types.SimpleNamespace(digest=PROFILE.digest, build_id=Mock(return_value='7' * 40),
                                         verified_packaged_roles=package_bind)
            spec = Mock()
            environment = {'CANDIDATE_DIR': str(candidate), 'TARGET': 'x86_64-unknown-linux-gnu',
                           'TMPDIR': str(temporary), 'PROFILE_PREFLIGHT_OUT': str(output),
                           'CANDIDATE_SHA': 'a' * 40, 'GITHUB_ENV': str(root / 'env')}
            with patch.dict(os.environ, environment), patch.object(importlib.util, 'spec_from_file_location', return_value=spec), patch.object(importlib.util, 'module_from_spec', return_value=fake), patch.object(subprocess, 'run') as run, patch.object(subprocess, 'check_output', return_value='a' * 40 + '\n'):
                exec(compile(code, 'workflow-embedded-packaged-preflight', 'exec'), {})
            descriptor = json.loads((temporary / 'tl775-owned-profile-binaries.json').read_text())
            self.assertEqual(descriptor['roles'], roles)
            package_bind.assert_called_once()
            arguments = package_bind.call_args.args
            self.assertEqual(arguments[0], release)
            self.assertEqual(arguments[2], app)
            self.assertEqual(arguments[3], PROFILE.digest(app))
            self.assertEqual(arguments[4:6], ('a' * 40, 'a' * 40))
            run.assert_called_once()
            self.assertEqual(run.call_args.args[0], [str(app), '--appimage-extract'])
            self.assertEqual(run.call_args.kwargs['timeout'], 60)

    def elf_fixture(self, code=b'CODE', address=0x401000, extra=b''):
        data = bytearray(128 + len(code))
        data[:7] = b'\x7fELF\x02\x01\x01'
        struct.pack_into('<HHI', data, 16, 3, 62, 1)
        struct.pack_into('<Q', data, 32, 64)
        struct.pack_into('<HH', data, 54, 56, 1)
        struct.pack_into('<IIQQQQQQ', data, 64, 1, 5, 128, address, address, len(code), len(code), 4096)
        data[128:] = code
        return bytes(data) + extra

    def packaged_fixture(self, root):
        release, appdir, image = root / 'release', root / 'extracted', root / 'Control.AppImage'
        release.mkdir(); (appdir / 'usr/bin').mkdir(parents=True)
        image.write_bytes(b'exact frozen bundle')
        for name in ('light-headless', 'viz-renderer'):
            (release / name).write_bytes(self.elf_fixture(extra=b'original symbols'))
            packaged = appdir / 'usr/bin' / name
            packaged.write_bytes(self.elf_fixture(extra=b'packaged metadata'))
            packaged.chmod(0o755)
        return release, appdir, image

    def test_packaged_provenance_allows_only_noncode_byte_drift_with_same_build_identity(self):
        with tempfile.TemporaryDirectory() as directory:
            release, appdir, image = self.packaged_fixture(Path(directory))
            with patch.object(PROFILE, 'build_id', return_value='b' * 40), patch.object(PROFILE, 'defined_function_count', return_value=5):
                proof = PROFILE.verified_packaged_roles(release, appdir, image, PROFILE.digest(image), 'a' * 40, 'a' * 40, 99)
            self.assertTrue(proof['source_verified'])
            self.assertTrue(proof['fixed_packaged_paths_verified'])
            self.assertEqual(len(proof['roles']), 2)
            for role, comparison in zip(proof['roles'], proof['comparisons']):
                self.assertFalse(comparison['full_bytes_equal'])
                self.assertTrue(comparison['build_ids_equal'])
                self.assertTrue(comparison['loaded_images_equal'])
                self.assertEqual(role['sha256'], comparison['packaged_sha256'])
                self.assertNotEqual(role['sha256'], comparison['cargo_sha256'])
            self.assertNotIn(str(directory), str(proof))
            self.assertNotIn('401000', str(proof))

    def test_packaged_code_or_load_address_change_never_authorizes_descriptor(self):
        for code, address in [(b'EVIL', 0x401000), (b'CODE', 0x402000)]:
            with self.subTest(code=code, address=address), tempfile.TemporaryDirectory() as directory:
                release, appdir, image = self.packaged_fixture(Path(directory))
                (appdir / 'usr/bin/light-headless').write_bytes(self.elf_fixture(code, address))
                with patch.object(PROFILE, 'build_id', return_value='b' * 40), patch.object(PROFILE, 'defined_function_count', return_value=5), self.assertRaises(PROFILE.PackagedBindingRefused) as error:
                    PROFILE.verified_packaged_roles(release, appdir, image, PROFILE.digest(image), 'a' * 40, 'a' * 40, 99)
                self.assertEqual(str(error.exception), 'packaged_role_code_or_build_identity_changed')
                self.assertFalse(error.exception.diagnostic['fixed_packaged_paths_verified'])

    def test_packaged_source_bundle_buildid_symbols_and_symlink_are_guarded(self):
        for case in ('source', 'bundle', 'buildid', 'symbols', 'symlink'):
            with self.subTest(case=case), tempfile.TemporaryDirectory() as directory:
                release, appdir, image = self.packaged_fixture(Path(directory))
                packaged = appdir / 'usr/bin/light-headless'
                if case == 'symlink':
                    packaged.unlink(); packaged.symlink_to(release / 'light-headless')
                def identity(path, deadline):
                    return ('c' if case == 'buildid' and Path(path) == packaged else 'b') * 40
                with patch.object(PROFILE, 'build_id', side_effect=identity), patch.object(PROFILE, 'defined_function_count', return_value=0 if case == 'symbols' else 5), self.assertRaises(PROFILE.PackagedBindingRefused):
                    PROFILE.verified_packaged_roles(release, appdir, image,
                        '9' * 64 if case == 'bundle' else PROFILE.digest(image),
                        'a' * 40, 'd' * 40 if case == 'source' else 'a' * 40, 99)

    def test_elf_fingerprint_rejects_unproven_architecture_and_truncated_loads(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'fixture'
            valid = self.elf_fixture()
            for blob in [b'not an ELF', valid[:90], valid[:18] + struct.pack('<H', 183) + valid[20:], valid[:-1]]:
                path.write_bytes(blob)
                with self.subTest(size=len(blob)), self.assertRaises(PROFILE.Refused):
                    PROFILE.loaded_image_fingerprint(path)

    def multi_load_elf_fixture(self):
        data = bytearray(268)
        data[:7] = b'\x7fELF\x02\x01\x01'
        struct.pack_into('<HHI', data, 16, 3, 62, 1)
        struct.pack_into('<Q', data, 32, 64)
        struct.pack_into('<HH', data, 54, 56, 3)
        for index, (flags, payload) in enumerate([(5, b'CODE'), (4, b'CONS'), (6, b'DATA')]):
            offset, address = 256 + index * 4, 0x401000 + index * 0x1000
            struct.pack_into('<IIQQQQQQ', data, 64 + index * 56, 1, flags, offset,
                             address, address, 4, 12 if index == 2 else 4, 4096)
            data[offset:offset + 4] = payload
        return bytes(data)

    def test_nonexecutable_loaded_constants_data_and_permissions_cannot_change(self):
        for mutation in ('readonly_constants', 'writable_data', 'readonly_permissions', 'zero_fill'):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory:
                release, appdir, image = self.packaged_fixture(Path(directory))
                original = self.multi_load_elf_fixture()
                for name in ('light-headless', 'viz-renderer'):
                    (release / name).write_bytes(original)
                    (appdir / 'usr/bin' / name).write_bytes(original)
                changed = bytearray(original)
                if mutation == 'readonly_constants':
                    changed[260:264] = b'EVIL'
                elif mutation == 'writable_data':
                    changed[264:268] = b'EVIL'
                elif mutation == 'readonly_permissions':
                    struct.pack_into('<I', changed, 64 + 56 + 4, 6)
                else:
                    struct.pack_into('<Q', changed, 64 + 2 * 56 + 40, 16)
                (appdir / 'usr/bin/light-headless').write_bytes(changed)
                with patch.object(PROFILE, 'build_id', return_value='b' * 40), patch.object(PROFILE, 'defined_function_count', return_value=5), self.assertRaises(PROFILE.PackagedBindingRefused):
                    PROFILE.verified_packaged_roles(release, appdir, image, PROFILE.digest(image), 'a' * 40, 'a' * 40, 99)

    def test_elf_header_section_offset_entrypoint_and_program_alignment_are_not_masked(self):
        for field, offset, value in [('section_offset', 40, 256), ('entrypoint', 24, 0x401000),
                                     ('program_alignment', 64 + 48, 8192)]:
            with self.subTest(field=field), tempfile.TemporaryDirectory() as directory:
                release, appdir, image = self.packaged_fixture(Path(directory))
                packaged = appdir / 'usr/bin/light-headless'
                data = bytearray(packaged.read_bytes())
                struct.pack_into('<Q', data, offset, value)
                packaged.write_bytes(data)
                with patch.object(PROFILE, 'build_id', return_value='b' * 40), patch.object(PROFILE, 'defined_function_count', return_value=5), self.assertRaises(PROFILE.PackagedBindingRefused):
                    PROFILE.verified_packaged_roles(release, appdir, image, PROFILE.digest(image), 'a' * 40, 'a' * 40, 99)

    def requests(self):
        return [{'role': role, 'target': {'sha256': 'a' * 64}, 'target_build_id': 'b' * 40}
                for role in ('server', 'native_renderer')]

    def test_both_role_cleanup_receipts_required_even_after_first_failure(self):
        calls = []
        def attempt(request, privileged):
            calls.append((request['role'], privileged))
            if request['role'] == 'server':
                raise BrokenPipeError('private error must not be published')
            return {'status': 'CAPABLE', 'recorder_gone': True}
        with patch.object(PROFILE, 'collect_attempt', side_effect=attempt):
            reports, clean = PROFILE.collect_owned_requests(self.requests())
        self.assertFalse(clean)
        self.assertEqual(len(reports), 1)
        self.assertEqual(set(calls), {('server', False), ('native_renderer', False)})
        self.assertNotIn('private error', str(reports))

    def test_only_permission_denied_verified_cleanup_allows_single_sudo_attempt(self):
        calls = []
        def attempt(request, privileged):
            calls.append((request['role'], privileged))
            return {'status': 'CAPABLE', 'recorder_gone': True} if privileged else {'status': 'STOP', 'code': 'perf_permission_denied', 'recorder_gone': True}
        with patch.object(PROFILE, 'collect_attempt', side_effect=attempt):
            reports, clean = PROFILE.collect_owned_requests(self.requests())
        self.assertTrue(clean)
        self.assertEqual(len(reports), 2)
        self.assertEqual(set(calls), {(role, elevated) for role in ('server', 'native_renderer') for elevated in (False, True)})
        for terminal in [{'status': 'STOP', 'code': 'other', 'recorder_gone': True},
                         {'status': 'STOP', 'code': 'perf_permission_denied'}]:
            with self.subTest(terminal=terminal), patch.object(PROFILE, 'collect_attempt', return_value=terminal) as attempt:
                _, clean = PROFILE.collect_owned_requests(self.requests())
                self.assertEqual(clean, terminal.get('recorder_gone') is True)
                self.assertEqual(attempt.call_count, 2)

    def test_no_actor_manifest_refusal_is_clean_but_missing_collect_receipt_is_not(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            request, output = root / 'request.json', root / 'receipt.json'
            request.write_text('{}')
            with patch.object(PROFILE, 'artifact_path', return_value=root), patch.object(PROFILE.sys, 'platform', 'linux'), patch.object(PROFILE.os, 'getuid', return_value=1001), patch.object(PROFILE.subprocess, 'Popen') as popen:
                self.assertEqual(PROFILE.owned_app(request, output), 2)
                self.assertTrue(__import__('json').loads(output.read_text())['recorder_cleanup_verified'])
                popen.assert_not_called()


if __name__ == "__main__":
    unittest.main()
