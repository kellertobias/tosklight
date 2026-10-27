"""CPU-only offline trace regressions; scratch files follow canonical artifact paths."""
import contextlib
import importlib.util
import io
from pathlib import Path
import tempfile
import unittest
import xml.etree.ElementTree as ET

from artifact_paths import artifact_path

SPEC = importlib.util.spec_from_file_location("stage_gpu_intervals", Path(__file__).with_name("stage-gpu-intervals.py"))
COLLECTOR = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(COLLECTOR)
COLUMNS = ["start", "duration", "channel-name", "event-depth", "event-label", "state", "process", "gpu"]


def row(start=0, duration=10, pid=10, channel="Fragment", depth=0, state="Active", device="Test GPU", **replace):
    fields = {
        "start": f"<start-time>{start}</start-time>",
        "duration": f"<duration>{duration}</duration>",
        "channel-name": f"<gpu-channel-name>{channel}</gpu-channel-name>",
        "event-depth": f"<metal-nesting-level>{depth}</metal-nesting-level>",
        "event-label": "<formatted-label>named pass, not GPU Execution</formatted-label>",
        "state": f"<gpu-state>{state}</gpu-state>",
        "process": f"<process><pid>{pid}</pid></process>",
        "gpu": f"<metal-device-name>{device}</metal-device-name>",
    }
    fields.update(replace)
    return fields


class IntervalsTest(unittest.TestCase):
    def setUp(self):
        root = artifact_path("LIGHT_TMP_DIR", "TMP_ROOT")
        root.mkdir(parents=True, exist_ok=True)
        self.scratch = tempfile.TemporaryDirectory(prefix="gpu-interval-unit-", dir=root)
        self.addCleanup(self.scratch.cleanup)
        self.path = Path(self.scratch.name) / "intervals.xml"

    def write(self, rows, columns=None):
        columns = COLUMNS if columns is None else columns
        schema = "".join(f"<col><mnemonic>{column}</mnemonic></col>" for column in columns)
        definitions, device_ids, bodies = [], {}, []
        for fields in rows:
            fields = dict(fields)
            gpu = ET.fromstring(fields["gpu"])
            if gpu.tag == "metal-device-name" and not gpu.attrib:
                label = gpu.text
                if label not in device_ids:
                    identity = f"fixture-gpu-{len(device_ids)}"
                    device_ids[label] = identity
                    gpu.set("id", identity)
                    definitions.append(ET.tostring(gpu, encoding="unicode"))
                fields["gpu"] = f'<metal-device-name ref="{device_ids[label]}"/>'
            bodies.append("<row>" + "".join(fields[column] for column in columns) + "</row>")
        body = "".join(bodies)
        self.path.write_text(f'<trace-query-result>{"".join(definitions)}<node><schema name="metal-gpu-intervals">{schema}</schema>{body}</node></trace-query-result>')
        return self.path

    def summarize(self, rows, pids=(10,), start=0, end=100, columns=None):
        return COLLECTOR.summarize(self.write(rows, columns), pids, start, end)

    def test_refs_nested_definition_and_dynamic_column_order(self):
        first = row(**{"event-label": '<formatted-label id="label"><process id="p"><pid id="pid">10</pid></process></formatted-label>',
                       "process": '<process ref="p"/>', "gpu": '<metal-device-name ref="device"/>'})
        second = row(start=10, **{"process": '<process><pid ref="pid"/></process>',
                                 "gpu": '<metal-device-name id="device">Test GPU</metal-device-name>'})
        result = self.summarize([first, second], columns=list(reversed(COLUMNS)))
        self.assertEqual(result["devices"][0]["activeUnionNs"], 20)
        self.assertEqual(result["scope"]["activeRowsInWindow"], 2)

    def test_union_deduplicates_nesting_channels_process_overlap_and_repeated_rows(self):
        rows = [row(duration=50), row(start=10, duration=20, depth=2), row(duration=50),
                row(start=30, duration=40, channel="Vertex"), row(start=60, duration=30, pid=11),
                row(duration=100, pid=99), row(duration=100, pid=99, state="Scheduled")]
        result = self.summarize(rows, pids=(10, 11))
        device = result["devices"][0]
        self.assertEqual(device["activeUnionNs"], 90)
        self.assertEqual(device["channelUnionSumNs"], 120)
        self.assertEqual(result["scope"]["rawDurationSumNs"], 190)
        self.assertAlmostEqual(device["activeWallMillisPerSecond"], 900)
        self.assertAlmostEqual(device["channelBusyMillisPerSecond"], 1200)

    def test_common_window_clips_execution_and_keeps_devices_separate(self):
        result = self.summarize([row(duration=40), row(start=35, duration=30),
                                 row(start=25, duration=10, device="Second GPU")], start=20, end=50)
        self.assertEqual({device["device"]: device["activeUnionNs"] for device in result["devices"]},
                         {"Second GPU": 10, "Test GPU": 30})
        self.assertEqual(result["window"]["wallSeconds"], 30 / 1e9)

    def test_same_gpu_names_with_distinct_resolved_ids_remain_separate(self):
        result = self.summarize([
            row(duration=30, gpu='<metal-device-name id="physical-a">Same GPU</metal-device-name>'),
            row(duration=40, gpu='<metal-device-name id="physical-b">Same GPU</metal-device-name>'),
            row(start=20, duration=30, gpu='<metal-device-name ref="physical-a"/>')])
        self.assertEqual({device["deviceXmlId"]: device["activeUnionNs"] for device in result["devices"]},
                         {"physical-a": 50, "physical-b": 40})
        self.assertEqual({device["device"] for device in result["devices"]}, {"Same GPU"})
        for state in ["Unknown", "Idle", "active", ""]:
            with self.subTest(state=state), self.assertRaisesRegex(ValueError, "Unknown selected GPU state"):
                self.summarize([row(), row(state=state)])

    def test_missing_pid_or_missing_window_evidence_is_error_not_zero(self):
        for pids, start, end in [((10, 11), 0, 100), ((10,), 20, 30)]:
            with self.subTest(pids=pids, start=start), self.assertRaisesRegex(ValueError, "No selected Active"):
                self.summarize([row()], pids, start, end)
        with self.assertRaisesRegex(ValueError, "Unknown selected GPU state"):
            self.summarize([row(state="Scheduled")])

    def test_unattributed_rows_do_not_become_selected_process_work(self):
        result = self.summarize([row(), row(duration=100, process="<sentinel/>")])
        self.assertEqual(result["scope"]["unattributedRows"], 1)
        self.assertEqual(result["devices"][0]["activeUnionNs"], 10)

    def test_unresolved_cyclic_and_duplicate_ids_fail_even_when_unselected(self):
        bad = [row(pid=99, process='<process ref="missing"/>'),
               row(process='<process id="a" ref="b"/><process id="b" ref="a"/>'),
               row(process='<process id="same"><pid>10</pid></process>', gpu='<metal-device-name id="same">Test GPU</metal-device-name>')]
        for fields in bad:
            with self.subTest(fields=fields), self.assertRaises(ValueError):
                self.summarize([fields])

    def test_invalid_schema_cell_count_pid_and_timestamps_fail(self):
        for fields in [row(start=-1), row(duration=-1), row(duration="nan"), row(pid="not-pid"),
                       row(process="<process/>"), row(gpu="<sentinel/>"), row(start=2**63 - 1)]:
            with self.subTest(fields=fields), self.assertRaises(ValueError):
                self.summarize([fields])
        with self.assertRaisesRegex(ValueError, "Unknown schema"):
            self.summarize([row(extra="<uint64>3</uint64>")], columns=COLUMNS + ["extra"])
        with self.assertRaisesRegex(ValueError, "required schema"):
            self.summarize([row()], columns=COLUMNS[:-1])
        with self.assertRaisesRegex(ValueError, "cell count"):
            self.summarize([row(process="<process><pid>10</pid></process><sentinel/>")])

    def test_invalid_windows_pid_lists_and_xml_declarations_fail(self):
        self.write([row()])
        for pids, start, end in [([], 0, 100), ([1], 0, 100), ([10, 10], 0, 100),
                                 ([10], -1, 100), ([10], 10, 10), ([10], 100, 0)]:
            with self.subTest(pids=pids, start=start), self.assertRaises(ValueError):
                COLLECTOR.summarize(self.path, pids, start, end)
        self.path.write_text('<!DOCTYPE x><trace-query-result/>')
        with self.assertRaisesRegex(ValueError, "declarations"):
            COLLECTOR.load_table(self.path)

    def test_actual_duration_precision_and_cli_window_exclusivity(self):
        self.assertEqual(COLLECTOR.duration_ns("11.000456"), 11_000_456_000)
        for invalid in ["nan", "inf", "0", "-1", "0.0000000001", "word"]:
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                COLLECTOR.duration_ns(invalid)
        self.write([row()])
        base = ["--input", str(self.path), "--owned-pid", "10"]
        with contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(COLLECTOR.main(base), 1)
            self.assertEqual(COLLECTOR.main(base + ["--trace-duration-seconds", "1", "--window-start-ns", "0"]), 1)
        with contextlib.redirect_stdout(io.StringIO()) as output:
            self.assertEqual(COLLECTOR.main(base + ["--trace-duration-seconds", "1"]), 0)
        self.assertIn('"activeUnionNs": 10', output.getvalue())


if __name__ == "__main__":
    unittest.main()
