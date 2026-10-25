#!/usr/bin/env python3
"""Offline, process-scoped Metal execution intervals; never a display or utilization metric."""
import argparse
from collections import defaultdict
from decimal import Decimal, InvalidOperation
import json
from pathlib import Path
import sys
import xml.etree.ElementTree as ET

MAX_INPUT_BYTES = 128 * 1024 * 1024
MAX_ROWS = 250_000
MAX_IDS = 2_000_000
KNOWN_COLUMNS = {
    "start", "duration", "channel-name", "frame-number", "start-latency", "event-depth",
    "event-label", "state", "connection-UUID", "color", "process", "gpu", "channel-subtitle",
    "iosurface-accesses", "bytes", "cmdbuffer-id", "encoder-id", "gpu-submission-id",
}
REQUIRED_COLUMNS = {"start", "duration", "channel-name", "state", "process", "gpu"}
# Active is the only state confirmed in the retained Metal export. No unvalidated state is
# interpreted as idle: future schemas must explicitly establish which states can be excluded.
SUPPORTED_STATES = {"Active"}


def integer(value, label, minimum=0):
    try:
        parsed = int(value)
    except (TypeError, ValueError) as error:
        raise ValueError(f"Invalid {label}: {value!r}") from error
    if str(parsed) != str(value).strip() or parsed < minimum or parsed > (1 << 63) - 1:
        raise ValueError(f"Invalid {label}: {value!r}")
    return parsed


def union_ns(intervals):
    """Merge repeated, nested and overlapping intervals, including across GPU channels."""
    total, previous_end = 0, 0
    for start, end in sorted(intervals):
        total += max(0, end - max(start, previous_end))
        previous_end = max(previous_end, end)
    return total


def load_table(path):
    if Path(path).stat().st_size > MAX_INPUT_BYTES:
        raise ValueError("Input exceeds bounded XML size")
    data = Path(path).read_bytes()
    if b"<!DOCTYPE" in data.upper() or b"<!ENTITY" in data.upper():
        raise ValueError("XML document/entity declarations are unsupported")
    root = ET.fromstring(data)
    ids = {}
    for element in root.iter():
        identity = element.get("id")
        if identity is not None:
            if identity in ids:
                raise ValueError(f"Duplicate XML id {identity}")
            ids[identity] = element
            if len(ids) > MAX_IDS:
                raise ValueError("Input exceeds bounded XML identity count")

    def resolve(element):
        seen = set()
        while element.get("ref") is not None:
            reference = element.get("ref")
            if reference in seen or reference not in ids:
                raise ValueError(f"Unresolved or cyclic XML ref {reference}")
            seen.add(reference)
            element = ids[reference]
        return element

    for element in root.iter():
        resolve(element)  # Reject unresolved references even in unselected/unused columns.
    tables = [node for node in root.iter("node")
              if node.find("schema") is not None
              and node.find("schema").get("name") == "metal-gpu-intervals"]
    if len(tables) != 1:
        raise ValueError("Expected exactly one metal-gpu-intervals table")
    table = tables[0]
    columns = [column.findtext("mnemonic") for column in table.find("schema").findall("col")]
    if len(columns) != len(set(columns)) or not REQUIRED_COLUMNS.issubset(columns):
        raise ValueError("Duplicate or missing required schema columns")
    unknown = set(columns) - KNOWN_COLUMNS
    if unknown:
        raise ValueError(f"Unknown schema columns: {sorted(unknown)}")
    rows = table.findall("row")
    if len(rows) > MAX_ROWS:
        raise ValueError("Input exceeds bounded XML row count")
    return columns, rows, resolve


def summarize(path, owned_pids, window_start_ns, window_end_ns):
    pids = [integer(pid, "owned PID", 2) for pid in owned_pids]
    if not pids or len(pids) != len(set(pids)):
        raise ValueError("Explicit unique owned PIDs are required")
    start = integer(window_start_ns, "window start")
    end = integer(window_end_ns, "window end")
    if end <= start:
        raise ValueError("Common observation window must have positive duration")
    columns, rows, resolve = load_table(path)
    by_device = defaultdict(lambda: defaultdict(list))
    scope = {pid: {"rowsObserved": 0, "activeRowsInWindow": 0} for pid in pids}
    unattributed, active_rows, raw_ns = 0, 0, 0
    for row in rows:
        cells = list(row)
        if len(cells) != len(columns):
            raise ValueError("Row cell count disagrees with schema (sentinels must retain their position)")
        fields = {key: resolve(cell) for key, cell in zip(columns, cells)}
        first = integer(fields["start"].text, "interval start")
        duration = integer(fields["duration"].text, "interval duration")
        if first + duration > (1 << 63) - 1:
            raise ValueError("Interval end exceeds supported timestamp range")
        process = fields["process"]
        if process.tag == "sentinel":
            unattributed += 1
            continue
        pid_fields = process.findall("pid")
        if len(pid_fields) != 1:
            raise ValueError("Process cell lacks an unambiguous PID")
        pid = integer(resolve(pid_fields[0]).text, "process PID", 0)
        if pid not in scope:
            continue
        scope[pid]["rowsObserved"] += 1
        state = (fields["state"].text or "").strip()
        if state not in SUPPORTED_STATES:
            raise ValueError(f"Unknown selected GPU state {state!r}; only Active is validated")
        left, right = max(start, first), min(end, first + duration)
        if right <= left:
            continue
        device = (fields["gpu"].text or fields["gpu"].get("fmt", "")).strip()
        device_id = fields["gpu"].get("id")
        channel = (fields["channel-name"].text or "").strip()
        if not device or not device_id or not channel:
            raise ValueError("Selected execution lacks canonical XML GPU id or device/channel label")
        by_device[(device_id, device)][channel].append((left, right))
        scope[pid]["activeRowsInWindow"] += 1
        active_rows += 1
        raw_ns += right - left
    missing = [pid for pid, evidence in scope.items() if not evidence["activeRowsInWindow"]]
    if missing:
        raise ValueError(f"No selected Active execution evidence in common window for PIDs {missing}")
    wall_seconds = (end - start) / 1_000_000_000
    devices = []
    for (device_id, device), channels in sorted(by_device.items()):
        channel_totals = {name: union_ns(intervals) for name, intervals in sorted(channels.items())}
        active = union_ns([interval for intervals in channels.values() for interval in intervals])
        devices.append({
            "device": device, "deviceXmlId": device_id,
            "identityScope": "resolved GPU XML id within this export only; not stable across traces",
            "activeUnionNs": active,
            "activeWallMillisPerSecond": active / 1_000_000 / wall_seconds,
            "channels": [{"channel": name, "unionNs": total} for name, total in channel_totals.items()],
            "channelUnionSumNs": sum(channel_totals.values()),
            "channelBusyMillisPerSecond": sum(channel_totals.values()) / 1_000_000 / wall_seconds,
        })
    return {
        "schemaVersion": 1, "kind": "offline-owned-metal-execution-intervals",
        "input": str(Path(path).resolve()),
        "window": {"startNs": start, "endNs": end, "wallSeconds": wall_seconds},
        "scope": {"ownedPids": pids, "processes": [{"pid": pid, **scope[pid]} for pid in pids],
                  "tableRows": len(rows), "unattributedRows": unattributed,
                  "activeRowsInWindow": active_rows, "rawDurationSumNs": raw_ns,
                  "coverage": "selected processes observed; table completeness and ownership are caller evidence"},
        "devices": devices,
        "limitations": ["Execution interval union is not utilization, energy, displayed frames, or acceptance.",
                        "Channel sums can exceed wall time through concurrent execution.",
                        "Caller must validate PID lifetime/ownership and capture/window completeness.",
                        "Unselected processes, including WindowServer, are excluded."],
    }


def duration_ns(value):
    try:
        nanos = Decimal(value) * 1_000_000_000
    except InvalidOperation as error:
        raise ValueError("Invalid actual trace duration") from error
    if not nanos.is_finite() or nanos != nanos.to_integral_value():
        raise ValueError("Actual trace duration must be finite with nanosecond precision")
    return integer(str(int(nanos)), "actual trace duration", 1)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", required=True)
    parser.add_argument("--owned-pid", required=True, action="append", type=int)
    parser.add_argument("--trace-duration-seconds", help="Actual TOC duration, never requested time limit")
    parser.add_argument("--window-start-ns", type=int)
    parser.add_argument("--window-end-ns", type=int)
    args = parser.parse_args(argv)
    try:
        if args.trace_duration_seconds is not None:
            if args.window_start_ns is not None or args.window_end_ns is not None:
                raise ValueError("Choose actual trace duration OR explicit common window")
            start, end = 0, duration_ns(args.trace_duration_seconds)
        else:
            if args.window_start_ns is None or args.window_end_ns is None:
                raise ValueError("Actual trace duration or both common window endpoints are required")
            start, end = args.window_start_ns, args.window_end_ns
        result = summarize(args.input, args.owned_pid, start, end)
        print(json.dumps(result, indent=2))
        return 0
    except (ValueError, OSError, ET.ParseError) as error:
        print(json.dumps({"status": "error", "reason": str(error)}), file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
