#!/usr/bin/env python3
"""Independent UDP evidence for one stable release-acceptance look (Python stdlib).

Author expectations from manufacturer manuals, never ToskLight profiles/API output.
Example JSON (Art-Net universes are zero-based wire Port-Addresses; sACN 1..63999):
{"cases": [{"id": "red", "protocol": "artnet", "universe": 0,
 "source": {"ip": "127.0.0.1"},
 "reference": {"url": "https://manufacturer.example/manual.pdf", "page": "12",
               "mode": "16 channel", "rationale": "Red slot at full; pan home"},
 "expect": [{"slots": [1], "min": 255, "max": 255},
            {"slots": [2, 4], "value": 32768, "byte_order": "big"}]}]}

--output is required, fresh, and below the canonical TEST_RESULTS path (resolve with
tools/artifact_paths.py; LIGHT_TEST_RESULTS_DIR/LIGHT_ARTIFACTS_DIR overrides work).
Run after setting the look through the UI/OSC. Example:
python3 tests/bench/release-dmx-acceptance.py --expectations CASE.json \
  --bind 127.0.0.1 --seconds 3 --output .artifacts/test/results/dmx-red
For sACN multicast, --interface must name the receiving IPv4 interface; memberships
are derived from sACN case universes. --settle-seconds excludes initial frames from
value assertions, but malformed packets always fail. Every eligible subsequent frame
must match; min-frames is per case. Preview/termination/nonzero-start-code frames
are retained but never pass ordinary DMX assertions. No merging or sync processing is
performed, so this proves individual wire payloads, not the receiver's rendered look.
Raw datagrams (hex) and decoded frames are retained in packets.jsonl; result.json
contains expectations, failures and counts. It does not prove physical color/aim.

Protocol references (reviewed 2026-10-08):
https://art-net.org.uk/downloads/art-net.pdf (ArtDmx packet definition, pp. 63-64)
https://github.com/ETCLabs/sACN/blob/main/src/sacn/pdu.c
https://github.com/ETCLabs/sACN/blob/main/src/sacn/private/common.h
Offsets below are independently implemented, not copied from application decoders.
"""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import ipaddress
import json
from pathlib import Path
import selectors
import socket
import sys
import time
import uuid

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tools"))
from artifact_paths import artifact_path


def u16(data, offset):
    return int.from_bytes(data[offset:offset + 2], "big")


def decode(protocol, data):
    """Return DMX frame, None for valid non-data traffic, or raise ValueError."""
    if protocol == "artnet":
        if len(data) < 10 or data[:8] != b"Art-Net\x00":
            raise ValueError("invalid Art-Net header")
        if int.from_bytes(data[8:10], "little") != 0x5000:
            return None  # ArtPoll/Reply/Sync etc. are not ArtDmx assertions.
        if len(data) < 18 or u16(data, 10) < 14:
            raise ValueError("invalid ArtDmx version/header")
        count = u16(data, 16)
        universe = int.from_bytes(data[14:16], "little")
        if not 2 <= count <= 512 or count % 2 or len(data) != 18 + count or universe > 32767:
            raise ValueError("invalid ArtDmx length/Port-Address")
        return dict(universe=universe, sequence=data[12], physical=data[13],
                    preview=False, terminated=False, start_code=0, slots=list(data[18:]))
    if len(data) < 38 or data[:4] != b"\x00\x10\x00\x00" or data[4:16] != b"ASC-E1.17\x00\x00\x00":
        raise ValueError("invalid E1.31 root header")
    root_vector = int.from_bytes(data[18:22], "big")
    if root_vector == 8:
        return None  # Extended synchronization/discovery, outside this validator.
    if root_vector != 4 or len(data) < 126:
        raise ValueError("invalid E1.31 data vector/header")
    for offset in (16, 38, 115):
        flags_length = u16(data, offset)
        if flags_length >> 12 != 7 or flags_length & 0xfff != len(data) - offset:
            raise ValueError("invalid E1.31 PDU flags/length")
    if int.from_bytes(data[40:44], "big") != 2 or data[117:123] != b"\x02\xa1\x00\x00\x00\x01":
        raise ValueError("invalid E1.31 framing/DMP fields")
    count, universe = u16(data, 123), u16(data, 113)
    if not 1 <= count <= 513 or len(data) != 125 + count or not 1 <= universe <= 63999:
        raise ValueError("invalid E1.31 count/universe")
    if data[108] > 200 or data[112] & 0x1f or b"\x00" not in data[44:108]:
        raise ValueError("invalid E1.31 priority/options/source name")
    return dict(universe=universe, sequence=data[111], cid=str(uuid.UUID(bytes=data[22:38])),
                source_name=data[44:108].split(b"\x00", 1)[0].decode("utf-8", errors="replace"),
                priority=data[108], sync_universe=u16(data, 109), force_sync=bool(data[112] & 0x20),
                preview=bool(data[112] & 0x80), terminated=bool(data[112] & 0x40),
                start_code=data[125], slots=list(data[126:]))


def load_cases(path):
    cases = json.loads(Path(path).read_text())["cases"]
    if not isinstance(cases, list) or not cases:
        raise ValueError("cases must be a nonempty list")
    ids = set()
    for case in cases:
        if not isinstance(case.get("id"), str) or not case["id"] or case["id"] in ids:
            raise ValueError("unique nonempty case IDs required")
        ids.add(case["id"])
        protocol = case["protocol"]
        limit = (0, 32767) if protocol == "artnet" else (1, 63999)
        if protocol not in ("artnet", "sacn") or type(case["universe"]) is not int or not limit[0] <= case["universe"] <= limit[1]:
            raise ValueError("invalid protocol/wire universe")
        ipaddress.IPv4Address(case["source"]["ip"])
        if "cid" in case["source"]:
            case["source"]["cid"] = str(uuid.UUID(case["source"]["cid"]))
        if set(case["source"]) - {"ip", "cid", "source_name"}:
            raise ValueError("unknown source filter")
        if protocol == "artnet" and set(case["source"]) != {"ip"}:
            raise ValueError("Art-Net has no CID/source-name field")
        ref = case["reference"]
        if not all(str(ref.get(k, "")).strip() for k in ("url", "page", "mode", "rationale")) or not ref["url"].startswith("https://"):
            raise ValueError("manufacturer HTTPS URL, page, mode and rationale required")
        if not case["expect"]:
            raise ValueError("empty expectations cannot pass")
        for expect in case["expect"]:
            slots = expect["slots"]
            if not isinstance(slots, list) or not slots or len(set(slots)) != len(slots) or any(type(s) is not int or not 1 <= s <= 512 for s in slots):
                raise ValueError("slots must be distinct one-based DMX addresses")
            if "value" in expect:
                if set(expect) != {"slots", "value", "byte_order"} or expect["byte_order"] not in ("big", "little") or not 1 <= len(slots) <= 4 or type(expect["value"]) is not int or not 0 <= expect["value"] < 256 ** len(slots):
                    raise ValueError("invalid multi-byte value")
            elif set(expect) != {"slots", "min", "max"} or type(expect["min"]) is not int or type(expect["max"]) is not int or not 0 <= expect["min"] <= expect["max"] <= 255:
                raise ValueError("invalid raw slot range")
    return cases


def matches(case, frame):
    return case["protocol"] == frame["protocol"] and case["universe"] == frame["universe"] and all(frame.get(k) == v for k, v in case["source"].items())


def violations(case, frame):
    failures = []
    for expect in case["expect"]:
        if max(expect["slots"]) > len(frame["slots"]):
            failures.append("expected slot absent from payload")
            continue
        values = bytes(frame["slots"][slot - 1] for slot in expect["slots"])
        if "value" in expect:
            actual = int.from_bytes(values, expect["byte_order"])
            if actual != expect["value"]:
                failures.append(f"slots {expect['slots']}: expected {expect['value']}, received {actual}")
        elif any(not expect["min"] <= value <= expect["max"] for value in values):
            failures.append(f"slots {expect['slots']}: expected {expect['min']}..{expect['max']}, received {list(values)}")
    return failures


def eligible(frame, elapsed, settle):
    return elapsed >= settle and not frame["preview"] and not frame["terminated"] and frame["start_code"] == 0


def completion_failures(received, counts, minimum):
    failures = []
    if not received:
        failures.append(dict(reason="no datagrams received"))
    for case_id, count in counts.items():
        if count < minimum:
            failures.append(dict(case=case_id, reason=f"expected source/universe has {count} eligible frames; need {minimum}"))
    return failures


def output_path(value):
    path = Path(value).expanduser().resolve()
    root = artifact_path("LIGHT_TEST_RESULTS_DIR", "TEST_RESULTS").resolve()
    if path == root or not path.is_relative_to(root):
        raise ValueError(f"output must be a fresh subdirectory of {root}")
    return path


def udp_port(value):
    port = int(value)
    if not 1 <= port <= 65535:
        raise argparse.ArgumentTypeError("UDP port must be between 1 and 65535")
    return port


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--expectations", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--bind", required=True, help="IPv4 bind address; 0.0.0.0 for multicast")
    parser.add_argument("--interface", help="IPv4 interface for sACN multicast membership")
    parser.add_argument("--artnet-port", type=udp_port, default=6454, help="Art-Net listening port; custom port requires matching UI Unicast route")
    parser.add_argument("--sacn-port", type=udp_port, default=5568, help="sACN listening port; custom port requires matching UI Unicast route")
    parser.add_argument("--seconds", type=float, default=3)
    parser.add_argument("--settle-seconds", type=float, default=0)
    parser.add_argument("--min-frames", type=int, default=3)
    args = parser.parse_args()
    try:
        cases = load_cases(args.expectations)
        out = output_path(args.output)
        ipaddress.IPv4Address(args.bind)
        if args.interface:
            ipaddress.IPv4Address(args.interface)
        if not 0 <= args.settle_seconds < args.seconds or args.min_frames < 1 or args.seconds > 3600:
            raise ValueError("invalid duration, settle duration or min-frames")
        out.mkdir(parents=True, exist_ok=False)
    except (ValueError, KeyError, TypeError, OSError) as exc:
        parser.error(str(exc))
    counts = {case["id"]: 0 for case in cases}
    failures = []
    selector = selectors.DefaultSelector()
    sockets = []
    started = time.monotonic()
    received = 0
    try:
        for protocol in sorted({case["protocol"] for case in cases}):
            sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
            sockets.append(sock)
            sock.bind((args.bind, args.artnet_port if protocol == "artnet" else args.sacn_port))
            if protocol == "sacn" and args.interface:
                for universe in sorted({case["universe"] for case in cases if case["protocol"] == "sacn"}):
                    group = f"239.255.{universe >> 8}.{universe & 255}"
                    sock.setsockopt(socket.IPPROTO_IP, socket.IP_ADD_MEMBERSHIP, socket.inet_aton(group) + socket.inet_aton(args.interface))
            selector.register(sock, selectors.EVENT_READ, protocol)
        with (out / "packets.jsonl").open("x") as log:
            while time.monotonic() - started < args.seconds:
                for key, _ in selector.select(min(0.2, max(0, args.seconds - (time.monotonic() - started)))):
                    data, peer = key.fileobj.recvfrom(65535)
                    elapsed = time.monotonic() - started
                    received += 1
                    record = dict(timestamp_utc=datetime.now(timezone.utc).isoformat(), elapsed_seconds=elapsed,
                                  protocol=key.data, ip=peer[0], port=peer[1], raw_hex=data.hex())
                    try:
                        decoded = decode(key.data, data)
                        record["frame"] = decoded
                        if decoded:
                            frame = dict(decoded, protocol=key.data, ip=peer[0])
                            if eligible(frame, elapsed, args.settle_seconds):
                                for case in cases:
                                    if matches(case, frame):
                                        counts[case["id"]] += 1
                                        for reason in violations(case, frame):
                                            failures.append(dict(case=case["id"], packet=received, reason=reason))
                    except ValueError as exc:
                        record["error"] = str(exc)
                        failures.append(dict(packet=received, reason=str(exc)))
                    log.write(json.dumps(record) + "\n")
    except (OSError, KeyboardInterrupt) as exc:
        failures.append(dict(reason=f"capture failed or interrupted: {exc}"))
    finally:
        selector.close()
        for sock in sockets:
            sock.close()
    failures.extend(completion_failures(received, counts, args.min_frames))
    result = dict(passed=not failures, cases=cases, counts=counts, received_datagrams=received,
                  failures=failures, bind=args.bind, multicast_interface=args.interface,
                  artnet_port=args.artnet_port, sacn_port=args.sacn_port,
                  duration_seconds=args.seconds, settle_seconds=args.settle_seconds)
    (out / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(dict(passed=result["passed"], counts=counts, failures=len(failures), output=str(out))))
    return 0 if result["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
