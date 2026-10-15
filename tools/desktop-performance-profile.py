"""Opt-in Linux CPU profiling: synthetic capability or exact owned-app observation.

No invocation of perf is authorized merely by importing or testing this module.
The public receipt contains only validated owned-target aggregates. Unvalidated
perf output stays in a no-follow, private spool and is never printed or uploaded.
"""

from __future__ import annotations

import argparse
import collections
import concurrent.futures
import datetime
import bisect
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import select
import shutil
import signal
import stat
import struct
import subprocess
import sys
import time
import uuid

sys.dont_write_bytecode = True
from artifact_paths import artifact_path

LIMIT = 64 * 1024 * 1024
PROBE_NAMES = ("tl775_profile_probe_parent", "tl775_profile_probe_leaf")
PROBE = r"""
#include <signal.h>
#include <stdint.h>
#include <unistd.h>
static volatile sig_atomic_t done;
static volatile uint64_t answer;
static void stop(int sig) { (void)sig; done = 1; }
__attribute__((noinline)) uint64_t tl775_profile_probe_leaf(uint64_t n) {
    volatile uint64_t frame_state = n;
    for (unsigned i = 0; i < 100000; ++i)
        frame_state = frame_state * 1664525 + 1013904223;
    return frame_state;
}
__attribute__((noinline)) void tl775_profile_probe_parent(void) {
    volatile uint64_t frame_result = 0;
    while (!done) {
        frame_result = tl775_profile_probe_leaf(answer);
        answer = frame_result;
    }
}
int main(void) {
    signal(SIGTERM, stop); signal(SIGINT, stop);
    write(1, "READY\n", 6);
    tl775_profile_probe_parent();
    return 0;
}
"""


class Refused(RuntimeError):
    """A fixed scope/capability code, safe to publish without tool output."""


class MachineFrameRefused(Refused):
    """Only boolean proofs about the two fixed functions in our compiled ELF."""

    def __init__(self, proof):
        super().__init__("expected_probe_machine_frame_unverified")
        self.diagnostic = proof


class CallchainRefused(Refused):
    """Only aggregate counts for the two controlled probe functions."""

    def __init__(self, sampled, leaf, parent, dual):
        super().__init__("symbolized_probe_callchain_unavailable")
        self.diagnostic = {"sampled": sampled, "leaf_present": leaf,
                           "parent_present": parent, "dual_present": dual}


class TypedRefused(Refused):
    """Fixed enum/boolean classifications only, never raw typed export content."""

    def __init__(self, code, event_kind, prefix_class, payload_matches, time_class):
        super().__init__(code)
        self.diagnostic = {"event_kind": event_kind, "prefix_class": prefix_class,
                           "payload_owner_matches": payload_matches, "time_class": time_class}


class CensusRefused(Refused):
    """Numeric record-header diagnostics only; no event payload is published."""

    def __init__(self, event, misc, length, counts, sample_type, flags, inventory=None, code="unsupported_or_lost_perf_record"):
        super().__init__(code)
        self.diagnostic = {
            "record_type": event, "record_misc": misc, "record_length": length,
            "preceding_record_counts": dict(counts),
            "attribute_sample_type": sample_type, "attribute_flags": flags,
        }
        if inventory is not None:
            self.diagnostic["header_inventory"] = inventory


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def task_identity(pid):
    root = Path(f"/proc/{int(pid)}")
    text = (root / "stat").read_text()
    fields = text[text.rindex(")") + 2 :].split()
    status = dict(
        line.split(":", 1) for line in (root / "status").read_text().splitlines()
        if ":" in line
    )
    return {
        "pid": int(pid), "ppid": int(fields[1]), "start": int(fields[19]),
        "uid": int(status["Uid"].split()[0]),
        "tgid": int(status["Tgid"].strip()),
    }


def process_identity(pid):
    identity = task_identity(pid)
    exe = Path(f"/proc/{pid}/exe")
    identity.update(exe=str(exe.resolve(strict=True)), sha256=digest(exe))
    return identity


def bounded_timeout(deadline, maximum):
    remaining = deadline - time.monotonic() if deadline is not None else maximum
    if remaining <= 0:
        raise Refused("global_preflight_deadline")
    return min(maximum, remaining)


def build_id(executable, deadline=None):
    result = subprocess.run(
        ["readelf", "-n", str(executable)], capture_output=True, check=True, timeout=bounded_timeout(deadline, 2),
        env=isolated_env(),
    )
    match = re.search(rb"Build ID: ([0-9a-f]+)", result.stdout)
    if not match:
        raise Refused("elf_build_id_unavailable")
    return match[1].decode()


def verify_probe_elf_symbols(executable, deadline):
    result = subprocess.run(["readelf", "--symbols", "--wide", str(executable)],
                            check=True, capture_output=True, text=True, env=isolated_env(),
                            timeout=bounded_timeout(deadline, 2))
    if len(result.stdout) > LIMIT:
        raise Refused("expected_probe_elf_symbols_unverified")
    found = set()
    for line in result.stdout.splitlines():
        columns = line.split()
        if len(columns) != 8 or columns[7] not in PROBE_NAMES:
            continue
        if (columns[3:6] == ["FUNC", "GLOBAL", "DEFAULT"] and columns[6].isdigit()
                and columns[2].isdigit() and int(columns[2]) > 0
                and re.fullmatch(r"[0-9a-fA-F]+", columns[1]) and int(columns[1], 16) > 0):
            found.add(columns[7])
    if found != set(PROBE_NAMES):
        raise Refused("expected_probe_elf_symbols_unverified")
    return True


def probe_instructions(text, name):
    if len(text) > 1024 * 1024:
        return []
    active = False
    instructions = []
    for line in text.splitlines():
        header = re.fullmatch(r"\s*[0-9a-fA-F]+ <([^>]+)>:\s*", line)
        if header:
            if active:
                break
            active = header[1] == name
            continue
        instruction = re.fullmatch(r"\s*[0-9a-fA-F]+:\s+([a-z][a-z0-9]*)\s*(.*?)\s*", line)
        if active and instruction:
            instructions.append((instruction[1], instruction[2]))
    return instructions


def entry_frame_verified(instructions):
    # Conservative x86_64 entry proof; no generic disassembler or unwinder.
    body = list(instructions)
    while body and body[0][0] in ("endbr64", "nop", "nopl", "nopw"):
        body.pop(0)
    return len(body) >= 2 and body[0] == ("push", "rbp") and body[1][0] == "mov" and re.sub(r"\s+", "", body[1][1]) == "rbp,rsp"


def verify_probe_machine_frame(executable, deadline):
    proof = {"x86_64_elf_verified": False, "leaf_body_decoded": False,
             "parent_body_decoded": False, "leaf_entry_frame_verified": False,
             "parent_entry_frame_verified": False, "parent_calls_leaf_verified": False,
             "verified": False}
    try:
        header = subprocess.run(["readelf", "--file-header", str(executable)],
                                check=True, capture_output=True, text=True, env=isolated_env(),
                                timeout=bounded_timeout(deadline, 2)).stdout
        proof["x86_64_elf_verified"] = (len(header) <= 65536
            and bool(re.search(r"Class:\s+ELF64\b", header))
            and bool(re.search(r"Data:.*little endian", header))
            and bool(re.search(r"Machine:\s+Advanced Micro Devices X86-64", header)))
        if not proof["x86_64_elf_verified"]:
            raise MachineFrameRefused(proof)
        bodies = {}
        for name in PROBE_NAMES:
            result = subprocess.run(["objdump", "--disassemble=" + name, "--no-show-raw-insn",
                                     "-M", "intel", str(executable)], check=True, capture_output=True,
                                    text=True, env=isolated_env(), timeout=bounded_timeout(deadline, 2))
            bodies[name] = probe_instructions(result.stdout, name)
        parent, leaf = [bodies[name] for name in PROBE_NAMES]
        proof.update(parent_body_decoded=bool(parent), leaf_body_decoded=bool(leaf),
                     parent_entry_frame_verified=entry_frame_verified(parent),
                     leaf_entry_frame_verified=entry_frame_verified(leaf),
                     parent_calls_leaf_verified=any(op == "call" and re.fullmatch(
                         r"[0-9a-fA-F]+ <" + PROBE_NAMES[1] + r">", operand) for op, operand in parent))
        proof["verified"] = all(proof[key] for key in proof if key != "verified")
        if not proof["verified"]:
            raise MachineFrameRefused(proof)
        return proof
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        raise MachineFrameRefused(proof) from error


def find_perf():
    explicit = os.environ.get("LIGHT_PERF_ELF")
    found = [Path(explicit)] if explicit else [Path(shutil.which("perf") or "/missing")]
    found += [Path("/usr/lib/linux-tools") / platform.release() / "perf"]
    for path in found:
        if path.is_file() and path.read_bytes()[:4] == b"\x7fELF":
            return str(path.resolve())
    raise Refused("perf_direct_elf_unavailable")


def same_process(expected):
    try:
        actual = process_identity(expected["pid"])
        return all(actual.get(key) == value for key, value in expected.items() if key != "inode") and ("inode" not in expected or fast_same_process(expected))
    except (OSError, ValueError):
        return False


def derived_uid(request):
    coordinator = request["coordinator"]
    if not same_process(coordinator):
        raise Refused("coordinator_identity_changed")
    uid = os.getuid()
    if uid == 0:
        try:
            uid = int(os.environ["SUDO_UID"])
            int(os.environ["SUDO_GID"])
        except (KeyError, ValueError):
            raise Refused("missing_sudo_provenance") from None
        if uid == 0:
            raise Refused("root_is_not_probe_owner")
    cursor = os.getppid()
    for _ in range(16):
        parent = process_identity(cursor)
        if cursor == coordinator["pid"]:
            if any(parent.get(k) != v for k, v in coordinator.items() if k != "inode") or parent["uid"] != uid:
                raise Refused("coordinator_uid_mismatch")
            return uid
        cursor = parent["ppid"]
        if cursor <= 1:
            break
    raise Refused("coordinator_not_invoking_ancestor")


def verify_probe(request, uid):
    target = request["target"]
    if not same_process(target) or target["uid"] != uid:
        raise Refused("target_identity_changed")
    if target["ppid"] != request["coordinator"]["pid"]:
        raise Refused("probe_not_direct_owned_child")
    if target["pid"] in (1, os.getpid()) or target["tgid"] != target["pid"]:
        raise Refused("invalid_probe_target")
    if request.get("helper_sha256") != digest(__file__):
        raise Refused("helper_source_changed")
    if request.get("probe_source_sha256") != hashlib.sha256(PROBE.encode()).hexdigest():
        raise Refused("wrong_synthetic_probe")
    if Path(target["exe"]).name != "cpu-probe":
        raise Refused("not_synthetic_probe_executable")
    if build_id(target["exe"], request["work_deadline"]) != request["target_build_id"]:
        raise Refused("probe_build_id_changed")
    tids = list(Path(f"/proc/{target['pid']}/task").iterdir())
    identities = {int(t.name): task_identity(int(t.name)) for t in tids}
    if set(identities) != {target["pid"]}:
        raise Refused("preflight_probe_must_be_single_threaded")
    return identities


def open_directory(path):
    """Anchor each component; never follow a symlink or repurpose HOME."""
    path = Path(path).absolute()
    fd = os.open("/", os.O_RDONLY | os.O_DIRECTORY)
    try:
        for component in path.parts[1:]:
            child = os.open(
                component, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=fd,
            )
            os.close(fd)
            fd = child
        return fd
    except BaseException:
        os.close(fd)
        raise


def private_spool(parent, name):
    anchor = open_directory(parent)
    try:
        os.mkdir(name, 0o700, dir_fd=anchor)
        fd = os.open(name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=anchor)
        info = os.fstat(fd)
        if info.st_uid != os.getuid() or stat.S_IMODE(info.st_mode) != 0o700:
            os.close(fd)
            raise Refused("spool_owner_or_mode")
        return anchor, fd
    except BaseException:
        os.close(anchor)
        raise


def clean_spool(anchor, fd, name):
    for entry in os.listdir(fd):
        info = os.stat(entry, dir_fd=fd, follow_symlinks=False)
        if not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid():
            raise Refused("spool_contains_unexpected_inode")
        os.unlink(entry, dir_fd=fd)
    held = os.fstat(fd)
    current = os.stat(name, dir_fd=anchor, follow_symlinks=False)
    if (held.st_dev, held.st_ino) != (current.st_dev, current.st_ino):
        raise Refused("spool_directory_replaced")
    os.rmdir(name, dir_fd=anchor)


def isolated_env():
    return {
        "PATH": "/usr/sbin:/usr/bin:/sbin:/bin", "LC_ALL": "C",
        "PERF_CONFIG_NOGLOBAL": "1", "PERF_CONFIG_NOSYSTEM": "1",
        "DEBUGINFOD_URLS": "",
    }


def perf_command(perf, pid):
    return [
        perf, "record", "--event", "cpu-clock:u", "--freq", "49",
        "--call-graph", "fp", "--timestamp", "--mmap-pages", "128",
        "--clockid", "mono",
        "--no-inherit", "--no-buildid-cache", "--no-bpf-event", "--synth=task,mmap", "--pid", str(pid),
        "--output", "perf.data",
    ]


def stop_owned(child, pidfd, deadline=None):
    for sig, timeout in ((signal.SIGINT, 3), (signal.SIGTERM, 2), (signal.SIGKILL, 1)):
        if child.poll() is not None:
            return child.returncode
        if deadline is not None and time.monotonic() >= deadline:
            if pidfd is None:
                child.kill()
            else:
                signal.pidfd_send_signal(pidfd, signal.SIGKILL)
            raise Refused("cleanup_deadline_no_exit_proof")
        try:
            if pidfd is None:
                # Still our unreaped Popen child: its PID cannot be recycled until
                # this Popen reaps it. Covers pidfd_open failure after exec.
                child.send_signal(sig)
            else:
                signal.pidfd_send_signal(pidfd, sig)
        except ProcessLookupError:
            return child.wait(timeout=bounded_timeout(deadline, timeout))
        try:
            return child.wait(timeout=bounded_timeout(deadline, timeout))
        except subprocess.TimeoutExpired:
            pass
    raise Refused("recorder_cleanup_failed")


def emit(value):
    print(json.dumps(value, separators=(",", ":")), flush=True)


def cancelled(_signal, _frame):
    raise Refused("cancelled_cleanup_required")


def run_perf_tool(perf, arguments, fd, deadline):
    result = subprocess.run(
        [perf, *arguments], cwd=f"/proc/self/fd/{fd}", pass_fds=(fd,),
        env=isolated_env(), capture_output=True, timeout=bounded_timeout(deadline, 5),
    )
    if result.returncode:
        raise Refused("perf_typed_export_unavailable")
    if len(result.stdout) > LIMIT:
        raise Refused("typed_export_too_large")
    return result.stdout.decode("utf-8", errors="strict")


def verify_perf_config(perf, fd, deadline):
    # Linux v6.8 util/config.c perf_config_system/global honor the two
    # PERF_CONFIG_NO* variables; builtin-config.c supports --list, not the Git
    # config command's --show-origin. No HOME override or config writes.
    config = run_perf_tool(perf, ["config", "--list"], fd, deadline)
    if config.strip():
        raise Refused("perf_configuration_not_isolated")


def online_cpu_map():
    text = Path("/sys/devices/system/cpu/online").read_text().strip()
    if not re.fullmatch(r"[0-9]+(?:-[0-9]+)?(?:,[0-9]+(?:-[0-9]+)?)*", text):
        raise Refused("unsupported_online_cpu_map")
    cpus = []
    for part in text.split(","):
        bounds = [int(value) for value in part.split("-")]
        first, last = bounds[0], bounds[-1]
        if last < first or last - first > 4096:
            raise Refused("unsupported_online_cpu_map")
        cpus.extend(range(first, last + 1))
    if not cpus or len(cpus) > 4096 or cpus != sorted(set(cpus)):
        raise Refused("unsupported_online_cpu_map")
    return cpus


def attribute_ids(data, attr_offset, attr_size):
    if attr_size < 64:
        raise Refused("invalid_id_index_attr_section")
    ids_offset, ids_size = struct.unpack_from("<QQ", data, attr_offset + attr_size - 16)
    if not ids_size or ids_size > 4096 * 8 or ids_size % 8 or ids_offset + ids_size > len(data):
        raise Refused("invalid_id_index_attr_ids")
    record_offset, record_size = struct.unpack_from("<QQ", data, 40)
    ids_end = ids_offset + ids_size
    if ids_offset < 104 or (ids_offset < attr_offset + attr_size and attr_offset < ids_end) or (ids_offset < record_offset + record_size and record_offset < ids_end):
        raise Refused("overlapping_id_index_attr_ids")
    nr = ids_size // 8
    ids = struct.unpack_from(f"<{nr}Q", data, ids_offset)
    if len(set(ids)) != nr or 0 in ids:
        raise Refused("invalid_id_index_attr_ids")
    return ids


def validate_id_index(data, offset, length, misc, attr_offset, attr_size, target_tid, cpus):
    if isinstance(target_tid, (list, tuple)):
        return validate_app_id_index(data, offset, length, misc, attr_offset, attr_size, target_tid, cpus)
    # Linux v6.8.12 synthetic-events.c writes one entry per evsel core ID.
    # lib/perf/evlist.c idx is the mmap/CPU-map index, NOT evsel index.
    if target_tid is None or not cpus or misc != 0 or length < 16:
        raise Refused("unverified_id_index_scope")
    nr = struct.unpack_from("<Q", data, offset + 8)[0]
    if not nr or length != 16 + 32 * nr:
        # Guest extensions are intentionally outside this single-host probe.
        raise Refused("invalid_or_guest_id_index_length")
    ids = attribute_ids(data, attr_offset, attr_size)
    if len(ids) != nr:
        raise Refused("invalid_id_index_attr_ids")
    entries = [struct.unpack_from("<QQQq", data, offset + 16 + 32 * i) for i in range(nr)]
    if {entry[0] for entry in entries} != set(ids):
        raise Refused("id_index_attr_id_mismatch")
    cpu_map = [-1] if nr == 1 and entries[0][2] == (1 << 64) - 1 else cpus
    if nr != len(cpu_map):
        raise Refused("id_index_cpu_map_mismatch")
    for event_id, idx, cpu, tid in entries:
        if tid != target_tid:
            raise Refused("foreign_id_index_tid")
        if idx >= nr or event_id != ids[idx] or cpu != cpu_map[idx] % (1 << 64):
            raise Refused("id_index_id_cpu_ring_mismatch")


def decode_cpu_map(payload):
    if len(payload) < 8 or len(payload) > 8192 or len(payload) % 8:
        raise Refused("invalid_cpu_map_length")
    kind = struct.unpack_from("<H", payload)[0]
    if kind == 0:
        nr = struct.unpack_from("<H", payload, 2)[0]
        used = 4 + 2 * nr
        if not nr or used > len(payload):
            raise Refused("invalid_cpu_map_list")
        cpus = list(struct.unpack_from(f"<{nr}H", payload, 4))
        cpus = [-1 if cpu == 65535 else cpu for cpu in cpus]
    elif kind == 1:
        nr, width = struct.unpack_from("<HH", payload, 2)
        if not nr or width not in (4, 8):
            raise Refused("invalid_cpu_map_mask")
        start = 6 if width == 4 else 10
        if width == 8 and any(payload[6:10]):
            raise Refused("invalid_cpu_map_padding")
        used = start + nr * width
        if used > len(payload):
            raise Refused("invalid_cpu_map_mask")
        bits = int.from_bytes(payload[start:used], "little")
        cpus = [i for i in range(nr * width * 8) if bits & (1 << i)]
    elif kind == 2:
        any_cpu, padding, first, last = struct.unpack_from("<BBHH", payload, 2)
        if any_cpu not in (0, 1) or padding or last < first or last - first > 4096:
            raise Refused("invalid_cpu_map_range")
        cpus = ([-1] if any_cpu else []) + list(range(first, last + 1))
        used = 8
    else:
        raise Refused("unsupported_cpu_map_encoding")
    if not cpus or len(cpus) > 4096 or cpus != sorted(set(cpus)) or len(payload) != (used + 7) // 8 * 8 or any(payload[used:]):
        raise Refused("invalid_cpu_map_padding_or_order")
    return cpus


def validate_metadata(event, record, ids, target_tid, cpus):
    _, misc, length = struct.unpack_from("<IHH", record)
    if misc or target_tid is None or not cpus:
        raise Refused("unverified_metadata_scope")
    if event in (68, 82):
        if length != 8:
            raise Refused("invalid_finished_marker")
        return "FINISHED_ROUND" if event == 68 else "FINISHED_INIT"
    if event == 73 and isinstance(target_tid, (list, tuple)):
        nr = struct.unpack_from("<Q", record, 8)[0] if length >= 16 else 0
        if nr != len(target_tid) or length != 16 + 24 * nr:
            raise Refused("invalid_application_thread_map")
        found = [struct.unpack_from("<Q", record, 16 + 24 * i)[0] for i in range(nr)]
        if len(set(found)) != nr or set(found) != set(target_tid):
            raise Refused("foreign_application_thread_map")
        return "THREAD_MAP"
    if event == 73:
        if length != 40 or struct.unpack_from("<QQ", record, 8) != (1, target_tid):
            raise Refused("foreign_or_invalid_thread_map")
        if record[24:] not in (b"\0" * 16, b"cpu-probe\0" + b"\0" * 6):
            raise Refused("unexpected_thread_map_comm")
        return "THREAD_MAP"
    if event == 74:
        mapped = decode_cpu_map(record[8:])
        if mapped != cpus and not (mapped == [-1] and len(ids) == (len(target_tid) if isinstance(target_tid, (list, tuple)) else 1)):
            raise Refused("foreign_cpu_map")
        return "CPU_MAP"
    if length < 24:
        raise Refused("invalid_event_update_length")
    subtype, event_id = struct.unpack_from("<QQ", record, 8)
    if not ids or event_id != ids[0]:
        raise Refused("foreign_event_update_id")
    if subtype == 0:
        if length != 48 or record[24:29] != b"msec\0" or any(record[29:]):
            raise Refused("unexpected_clock_unit_update")
        return "EVENT_UPDATE_UNIT"
    if subtype == 1:
        if length != 48 or struct.unpack_from("<d", record, 24)[0] != 1e-6 or any(record[32:]):
            raise Refused("unexpected_clock_scale_update")
        return "EVENT_UPDATE_SCALE"
    if subtype == 3:
        if decode_cpu_map(record[24:]) != cpus:
            raise Refused("foreign_event_update_cpu_map")
        return "EVENT_UPDATE_CPUS"
    raise Refused("unsupported_event_update_subtype")


def numeric_header_inventory(data, offset, end, attr_offset, attr_size, deadline):
    """Diagnostic headers only: unknown payloads are never exported or accepted."""
    counts = collections.Counter()
    updates = collections.Counter()
    total = 0
    try:
        ids = attribute_ids(data, attr_offset, attr_size)
    except Refused:
        ids = ()
    reason = None
    while offset < end:
        if total >= 131072 or (deadline is not None and time.monotonic() >= deadline):
            reason = "diagnostic_limit_or_deadline"
            break
        if offset + 8 > end:
            reason = "truncated_header"
            break
        event, misc, length = struct.unpack_from("<IHH", data, offset)
        if length < 8 or offset + length > end:
            reason = "invalid_record_bounds"
            break
        key = (event, misc, length)
        if key not in counts and len(counts) >= 128:
            reason = "diagnostic_distinct_header_limit"
            break
        counts[key] += 1
        if event == 78 and length >= 24:
            subtype, event_id = struct.unpack_from("<QQ", data, offset + 8)
            matched = bool(ids) and event_id == ids[0]
            update_key = (matched, subtype if matched else None)
            if update_key not in updates and len(updates) >= 128:
                reason = "diagnostic_subtype_limit"
                break
            updates[update_key] += 1
        offset += length
        total += 1
    return {"complete": reason is None, "incomplete_reason": reason, "records": total,
            "headers": [{"type": key[0], "misc": key[1], "length": key[2], "count": count} for key, count in counts.items()],
            "event_updates": [{"event_id_matches": key[0], "subtype": key[1], "count": count} for key, count in updates.items()]}


def event_census(data, target_tid=None, cpus=None, deadline=None):
    """Validate headers and proven owned synthetic metadata, never sample payloads.

    PERFILE2 layout: Linux tools/perf/util/header.h; known synthetic layouts:
    tools/lib/perf/include/perf/event.h. Installed perf decodes task/sample data.
    """
    if len(data) < 104 or data[:8] != b"PERFILE2":
        raise Refused("unsupported_perf_file_header")
    _, attr_size, attr_offset, attrs_bytes, offset, size = struct.unpack_from("<6Q", data, 8)
    if not attr_size or attrs_bytes != attr_size or attr_offset + attr_size > len(data):
        raise Refused("expected_one_perf_event")
    kind, length, config, _, sample_type, _, flags = struct.unpack_from("<II5Q", data, attr_offset)
    allowed = 1 | 2 | 4 | 32 | 64 | 128 | 256 | 65536
    if kind != 1 or config != 0 or length < 48:
        raise Refused("not_software_cpu_clock")
    if sample_type & ~allowed or sample_type & (2 | 4 | 32) != (2 | 4 | 32):
        raise Refused("unsafe_or_incomplete_sample_fields")
    if flags & (1 << 1) or not flags & (1 << 5) or not flags & (1 << 6):
        raise Refused("unsafe_perf_scope_flags")
    end = offset + size
    if end > len(data) or size > LIMIT:
        raise Refused("invalid_perf_data_bounds")
    names = {1: "MMAP", 3: "COMM", 4: "EXIT", 7: "FORK", 9: "SAMPLE", 10: "MMAP2"}
    counts = collections.Counter()
    inventory = numeric_header_inventory(data, offset, end, attr_offset, attr_size, deadline)
    if not inventory["complete"]:
        raise CensusRefused(0, 0, 0, counts, sample_type, flags, inventory, "header_inventory_incomplete")
    while offset < end:
        if deadline is not None and time.monotonic() >= deadline:
            raise Refused("metadata_validation_deadline")
        if offset + 8 > end:
            raise Refused("truncated_perf_record")
        event, misc, length = struct.unpack_from("<IHH", data, offset)
        if length < 8 or offset + length > end:
            raise Refused("invalid_perf_record_length")
        if event == 69:
            if counts["ID_INDEX"]:
                raise Refused("duplicate_id_index_record")
            validate_id_index(data, offset, length, misc, attr_offset, attr_size, target_tid, cpus)
            counts["ID_INDEX"] += 1
            offset += length
            continue
        if event in (68, 73, 74, 78, 82):
            try:
                ids = attribute_ids(data, attr_offset, attr_size)
                name = validate_metadata(event, data[offset:offset + length], ids, target_tid, cpus)
                if counts[name] and event != 68:
                    raise Refused("duplicate_synthetic_metadata")
            except Refused as error:
                raise CensusRefused(event, misc, length, counts, sample_type, flags, inventory, str(error)) from error
            counts[name] += 1
            offset += length
            continue
        if event not in names:
            raise CensusRefused(event, misc, length, counts, sample_type, flags, inventory)
        if event in (1, 9, 10) and misc & 7 != 2:
            raise Refused("non_user_sample_or_mapping")
        counts[names[event]] += 1
        offset += length
    return counts


def validate_export(text, census, target, tids, interval, own_resolver=None):
    """Decode installed-perf typed samples/task metadata without exporting raw text.

    Linux6.8.12 synthesized task records have zeroed sample-ID trailers. With
    sample_id_all, builtin-script prints that0/0/time0 prefix; actual identities
    remain in the event payload. Samples always require exact owned PID/TID.
    """
    counts = collections.Counter()
    samples = []
    current = None
    prefix = re.compile(r"^\s*(\d+)\s*/\s*(\d+)\s+(\d+\.\d+):\s+(.*)$")
    frame = re.compile(r"^\s+[0-9a-fA-F]+\s+.+\s+\(.+\)\s*$")
    for line in text.splitlines():
        if not line.strip():
            current = None
            continue
        match = prefix.match(line)
        if not match:
            if current is not None and frame.match(line):
                current.append(line.strip())
                continue
            raise TypedRefused("unsupported_typed_event_syntax", "UNKNOWN", "unparsed", None, "unparsed")
        pid, tid = int(match[1]), int(match[2])
        body = match[4]
        is_sample = body.startswith("cpu-clock:u:")
        event = re.match(r"PERF_RECORD_(COMM|MMAP2?|FORK|EXIT)\b(.*)", body)
        kind = "SAMPLE" if is_sample else event[1] if event else "UNKNOWN"
        prefix_class = "owned" if pid == target["pid"] and tid in tids else "zero_placeholder" if pid == tid == 0 else "foreign_or_unverified"
        seconds, nanos = match[3].split(".")
        if len(nanos) > 9:
            raise TypedRefused("unsupported_timestamp_precision", kind, prefix_class, None, "unparsed")
        timestamp = int(seconds) * 1_000_000_000 + int(nanos.ljust(9, "0"))
        time_class = "zero" if timestamp == 0 else "within_verified_lifetime" if interval[0] <= timestamp <= interval[1] else "outside_verified_lifetime"
        def refuse(code, payload=None):
            raise TypedRefused(code, kind, prefix_class, payload, time_class)
        if is_sample:
            if prefix_class != "owned":
                refuse("foreign_or_unverified_event_identity")
            if time_class != "within_verified_lifetime":
                refuse("event_outside_verified_target_lifetime")
            counts["SAMPLE"] += 1
            current = [body]
            samples.append(current)
            continue
        current = None
        if not event:
            refuse("unsupported_typed_metadata")
        synthetic = timestamp == 0 and kind in ("COMM", "MMAP", "MMAP2", "FORK")
        separator = ":" if kind in ("FORK", "EXIT") else "/"
        identities = [(int(owner), int(thread)) for owner, thread in
                      re.findall(rf"(?<!\d)(\d+)\s*{separator}\s*(\d+)(?!\d)", event[2])]
        if not identities:
            refuse("metadata_identity_not_decoded", False)
        owned = all(owner == target["pid"] and thread in tids for owner, thread in identities)
        # Synthesized FORK describes this probe's already-verified coordinator
        # parent. It does not authorize sampling that parent or arbitrary PIDs.
        if synthetic and kind == "FORK":
            owned = identities == [(target["pid"], target["pid"]), (target["ppid"], target["ppid"])]
            if own_resolver is not None and len(identities) == 2:
                child, parent = identities
                owned = child[0] == target["pid"] and child[1] in tids and parent == ((target["ppid"], target["ppid"]) if child[1] == target["pid"] else (target["pid"], target["pid"]))
        if not owned:
            refuse("foreign_metadata_identity", False)
        if prefix_class != "owned" and not (synthetic and prefix_class == "zero_placeholder"):
            refuse("foreign_or_unverified_event_identity", True)
        if not synthetic and time_class != "within_verified_lifetime":
            refuse("event_outside_verified_target_lifetime", True)
        counts[kind] += 1
    expected = census.copy()
    header_names = ("ID_INDEX", "EVENT_UPDATE_UNIT", "EVENT_UPDATE_SCALE", "EVENT_UPDATE_CPUS",
                    "THREAD_MAP", "CPU_MAP", "FINISHED_INIT", "FINISHED_ROUND")
    header_counts = {name: expected.pop(name) for name in header_names if name in expected}
    if counts != expected:
        raise Refused("typed_event_census_mismatch")
    if own_resolver is not None:
        return aggregate_owned_samples(samples, counts, header_counts, own_resolver)
    present = [[any(name in frame for frame in sample) for name in PROBE_NAMES] for sample in samples]
    leaf = sum(row[1] for row in present)
    parent = sum(row[0] for row in present)
    useful = sum(all(row) for row in present)
    if len(samples) < 10 or useful < 10:
        raise CallchainRefused(len(samples), leaf, parent, useful)
    return {"event_counts": dict(counts), "validated_header_records": header_counts,
            "samples": len(samples), "probe_chains": useful}


def write_receipt(fd, value):
    receipt = os.open("receipt.json", os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600, dir_fd=fd)
    with os.fdopen(receipt, "w") as stream:
        json.dump(value, stream)
        stream.flush()
        os.fsync(stream.fileno())


def recover_recorder(request):
    """Only an invocation ancestor may recover its exact recorded child identity."""
    derived_uid(request)
    validate_spool_request(request)
    name = f"tl775-probe-{request['nonce']}-{os.getuid()}"
    anchor = open_directory(request["spool_parent"])
    fd = os.open(name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=anchor)
    try:
        if os.fstat(fd).st_uid != os.getuid() or stat.S_IMODE(os.fstat(fd).st_mode) != 0o700:
            raise Refused("cleanup_spool_owner_mismatch")
        receipt_fd = os.open("receipt.json", os.O_RDONLY | os.O_NOFOLLOW, dir_fd=fd)
        with os.fdopen(receipt_fd) as stream:
            info = os.fstat(stream.fileno())
            if info.st_uid != os.getuid() or not stat.S_ISREG(info.st_mode) or info.st_nlink != 1:
                raise Refused("cleanup_receipt_owner_mismatch")
            receipt = json.load(stream)
        if receipt["coordinator"] != request["coordinator"] or receipt["nonce"] != request["nonce"]:
            raise Refused("cleanup_receipt_scope_mismatch")
        recorded = receipt["recorder"]
        try:
            pidfd = os.pidfd_open(recorded["pid"])
        except ProcessLookupError:
            pidfd = None
        if pidfd is not None:
            try:
                exited = bool(select.select([pidfd], [], [], 0)[0])
                if not exited and (process_identity(recorded["pid"]) != recorded or recorded["uid"] != os.getuid()):
                    raise Refused("cleanup_recorder_pid_reused")
                for sig, wait in (() if exited else ((signal.SIGINT, 3), (signal.SIGTERM, 2), (signal.SIGKILL, 1))):
                    signal.pidfd_send_signal(pidfd, sig)
                    if select.select([pidfd], [], [], bounded_timeout(request["overall_deadline"] - 4, wait))[0]:
                        break
                else:
                    if not exited:
                        raise Refused("cleanup_recorder_survived")
            finally:
                os.close(pidfd)
        clean_spool(anchor, fd, name)
    finally:
        os.close(fd)
        os.close(anchor)
    emit({"event": "terminal", "status": "STOP", "code": "recovered_aborted_helper", "recorder_gone": True})


def validate_spool_request(request):
    if not re.fullmatch(r"[0-9a-f]{32}", request["nonce"]):
        raise Refused("invalid_run_nonce")
    if Path(request["spool_parent"]) != artifact_path("LIGHT_TMP_DIR", "TMP_ROOT"):
        raise Refused("spool_not_canonical_tmp")


def recorder(request):
    if request["overall_deadline"] - time.monotonic() > 50:
        raise Refused("invalid_preflight_deadline")
    bounded_timeout(request["work_deadline"], 1)
    uid = derived_uid(request)
    application = request.get("kind") == "owned_app"
    tids = verify_app(request, uid) if application else verify_probe(request, uid)
    stable = fast_same_process if application else same_process
    own_resolver = app_symbol_resolver(request["target"], request["work_deadline"]) if application else None
    cpus = online_cpu_map()
    perf = str(Path(request["perf"]).resolve(strict=True))
    if digest(perf) != request["perf_sha256"] or build_id(perf, request["work_deadline"]) != request["perf_build_id"]:
        raise Refused("perf_executable_changed")
    validate_spool_request(request)
    name = f"tl775-probe-{request['nonce']}-{os.getuid()}"
    anchor, fd = private_spool(request["spool_parent"], name)
    target_fd = os.pidfd_open(request["target"]["pid"])
    child = None
    recorder_fd = None
    receipt = {"status": "STOP", "code": "not_started"}
    try:
        verify_perf_config(perf, fd, request["work_deadline"])
        output = os.open("recorder.log", os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600, dir_fd=fd)
        try:
            begin = time.monotonic_ns()
            child = subprocess.Popen(
                perf_command(perf, request["target"]["pid"]),
                cwd=f"/proc/self/fd/{fd}", pass_fds=(fd,), env=isolated_env(),
                stdin=subprocess.DEVNULL, stdout=output, stderr=output,
            )
        finally:
            os.close(output)
        recorder_fd = os.pidfd_open(child.pid)
        try:
            recorded = process_identity(child.pid)
        except (OSError, ValueError):
            log = Path(f"/proc/self/fd/{fd}/recorder.log").read_text()
            if re.search(r"Permission|Operation not permitted|Access.*denied|perf_event_paranoid", log, re.I):
                raise Refused("perf_permission_denied") from None
            raise Refused("perf_recorder_exited_early") from None
        if recorded["sha256"] != request["perf_sha256"]:
            raise Refused("recorder_executable_changed")
        write_receipt(fd, {"recorder": recorded, "coordinator": request["coordinator"], "nonce": request["nonce"]})
        emit({"event": "started", "spool": name})
        deadline = time.monotonic() + (10 if application else 2)
        if deadline > request["work_deadline"]:
            raise Refused("insufficient_budget_for_two_second_probe")
        next_thread_check = 0
        while time.monotonic() < deadline:
            if not stable(request["target"]) or select.select([target_fd], [], [], 0)[0]:
                raise Refused("target_died_or_changed_quarantined")
            if not stable(request["coordinator"]):
                raise Refused("coordinator_changed_quarantined")
            if not application or time.monotonic() >= next_thread_check:
                if application and set(int(t.name) for t in Path(f"/proc/{request['target']['pid']}/task").iterdir()) != set(tids):
                    raise Refused("thread_set_changed_quarantined")
                for tid, expected in tids.items():
                    if task_identity(tid) != expected:
                        raise Refused("thread_lifetime_changed_quarantined")
                next_thread_check = time.monotonic() + .25
            for entry in ("perf.data", "recorder.log"):
                try:
                    if os.stat(entry, dir_fd=fd, follow_symlinks=False).st_size > LIMIT:
                        raise Refused("recorder_output_limit")
                except FileNotFoundError:
                    pass
            if select.select([sys.stdin], [], [], .02)[0] and not os.read(sys.stdin.fileno(), 1):
                raise Refused("coordinator_pipe_closed")
            if child.poll() is not None:
                log = Path(f"/proc/self/fd/{fd}/recorder.log").read_text()
                if re.search(r"Permission|Operation not permitted|Access.*denied|perf_event_paranoid", log, re.I):
                    raise Refused("perf_permission_denied")
                raise Refused("perf_recorder_exited_early")
        stop_owned(child, recorder_fd, request["overall_deadline"] - 10)
        end = time.monotonic_ns()
        if not same_process(request["target"]) or not same_process(request["coordinator"]):
            raise Refused("post_capture_identity_changed_quarantined")
        if application:
            if verify_app(request, uid) != tids:
                raise Refused("post_capture_application_thread_identity_changed")
        raw_fd = os.open("perf.data", os.O_RDONLY | os.O_NOFOLLOW, dir_fd=fd)
        try:
            info = os.fstat(raw_fd)
            if not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid() or info.st_nlink != 1 or info.st_size > LIMIT:
                raise Refused("unexpected_perf_output_inode")
            raw = os.read(raw_fd, LIMIT + 1)
            if online_cpu_map() != cpus:
                raise Refused("online_cpu_map_changed")
            census = event_census(raw, sorted(tids) if application else request["target"]["pid"], cpus, request["work_deadline"])
            typed = run_perf_tool(perf, [
                "script", "--ns", "--show-task-events", "--show-mmap-events",
                "-F", "pid,tid,time,event,ip,sym,dso", "-i", "perf.data",
            ], fd, request["work_deadline"])
            result = validate_export(typed, census, request["target"], tids, (begin, end), own_resolver)
            if application and own_executable_maps(request["target"]) != own_resolver.maps:
                raise Refused("owned_elf_mapping_changed")
            final = os.fstat(raw_fd)
            if (final.st_dev, final.st_ino, final.st_size, final.st_mtime_ns) != (info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns):
                raise Refused("output_mutated_during_validation")
        finally:
            os.close(raw_fd)
        receipt = {"status": "CAPABLE", "recorder_uid": os.getuid(), **result}
    except (Refused, OSError, ValueError, subprocess.SubprocessError) as error:
        receipt = {"status": "STOP", "code": str(error) if isinstance(error, Refused) else "preflight_tool_or_scope_unavailable"}
        if isinstance(error, CensusRefused):
            receipt["record_header_diagnostic"] = error.diagnostic
        if isinstance(error, TypedRefused):
            receipt["typed_event_diagnostic"] = error.diagnostic
        if isinstance(error, CallchainRefused):
            receipt["callchain_diagnostic"] = error.diagnostic
    finally:
        if child is not None:
            stop_owned(child, recorder_fd, request["overall_deadline"] - 10)
        if recorder_fd is not None:
            os.close(recorder_fd)
        os.close(target_fd)
        clean_spool(anchor, fd, name)
        os.close(fd)
        os.close(anchor)
    emit({"event": "terminal", "recorder_gone": child is None or child.poll() is not None, **receipt})


def collect_attempt(request, privileged):
    command = [sys.executable, "-B", str(Path(__file__).resolve()), "_record"]
    if privileged:
        command = ["sudo", "-n", "--", *command]
    child = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
    try:
        child.stdin.write(json.dumps(request) + "\n")
        child.stdin.flush()
        terminal = None
        deadline = request["work_deadline"]
        while time.monotonic() < deadline:
            if select.select([child.stdout], [], [], .1)[0]:
                line = child.stdout.readline()
                if not line:
                    break
                event = json.loads(line)
                if event.get("event") == "terminal":
                    terminal = event
            if child.poll() is not None:
                break
        child.stdin.close()
        child.wait(timeout=bounded_timeout(request["overall_deadline"] - 10, 6))
        for line in child.stdout.read().splitlines():
            event = json.loads(line)
            if event.get("event") == "terminal":
                terminal = event
        if terminal is None or terminal.get("recorder_gone") is not True:
            recovery_command = command[:-1] + ["_cleanup"]
            recovery = subprocess.run(recovery_command, input=json.dumps(request) + "\n", text=True, capture_output=True, timeout=bounded_timeout(request["overall_deadline"] - 4, 6))
            if recovery.returncode:
                raise Refused("helper_cleanup_unverified")
            return {"status": "STOP", "code": "helper_aborted_recorder_recovered", "recorder_gone": True}
        if terminal.get("status") == "CAPABLE" and terminal.get("recorder_gone") is not True:
            raise Refused("successful_profile_cleanup_not_verified")
        return terminal
    finally:
        if not child.stdin.closed:
            child.stdin.close()
        try:
            if child.poll() is None:
                child.wait(timeout=bounded_timeout(request["overall_deadline"] - 10, 6))
        except subprocess.TimeoutExpired:
            # The pipe EOF requests exact-recorder cleanup; never kill by name.
            raise Refused("helper_cleanup_unverified") from None


def preflight(output):
    overall_deadline = time.monotonic() + 50
    work_deadline = overall_deadline - 16
    allowed_roots = [artifact_path("LIGHT_TMP_DIR", "TMP_ROOT"), artifact_path("LIGHT_TEST_RESULTS_DIR", "TEST_RESULTS")]
    if not any(output.resolve().is_relative_to(root.resolve()) for root in allowed_roots):
        raise Refused("public_receipt_not_in_canonical_artifacts")
    if sys.platform != "linux" or os.getuid() == 0:
        raise Refused("requires_ordinary_linux_ci_coordinator")
    for method in ("pidfd_open",):
        if not hasattr(os, method) or not hasattr(signal, "pidfd_send_signal"):
            raise Refused("pidfd_capability_unavailable")
    perf, compiler = find_perf(), shutil.which("cc")
    if not perf or not compiler:
        raise Refused("perf_or_compiler_unavailable_before_build")
    parent = artifact_path("LIGHT_TMP_DIR", "TMP_ROOT")
    parent.mkdir(parents=True, exist_ok=True)
    stage = parent / f"tl775-probe-source-{uuid.uuid4().hex}"
    stage.mkdir(mode=0o700)
    probe = None
    recorder_cleanup_verified = True
    try:
        source, binary = stage / "probe.c", stage / "cpu-probe"
        source.write_text(PROBE)
        subprocess.run([
            compiler, "-O1", "-g", "-fno-omit-frame-pointer", "-fno-optimize-sibling-calls",
            str(source), "-o", str(binary),
        ], check=True, capture_output=True, timeout=bounded_timeout(work_deadline, 5))
        symbols_verified = verify_probe_elf_symbols(binary, work_deadline)
        machine_frame_proof = verify_probe_machine_frame(binary, work_deadline)
        probe = subprocess.Popen([str(binary)], stdout=subprocess.PIPE, text=True)
        if not select.select([probe.stdout], [], [], bounded_timeout(work_deadline, 2))[0] or probe.stdout.readline() != "READY\n":
            raise Refused("synthetic_probe_not_ready")
        request = {
            "coordinator": process_identity(os.getpid()), "target": process_identity(probe.pid),
            "helper_sha256": digest(__file__), "probe_source_sha256": digest(source),
            "nonce": uuid.uuid4().hex, "spool_parent": str(parent),
            "perf": str(Path(perf).resolve()), "perf_sha256": digest(perf),
            "target_build_id": build_id(binary, work_deadline), "perf_build_id": build_id(perf, work_deadline),
            "work_deadline": work_deadline, "overall_deadline": overall_deadline,
        }
        try:
            attempts = [collect_attempt(request, False)]
            if attempts[-1].get("code") == "perf_permission_denied":
                attempts.append(collect_attempt(request, True))
        except BaseException:
            # No exception type is itself proof the recorder stopped. Preserve
            # the target/binary for constrained recovery or runner teardown.
            recorder_cleanup_verified = False
            raise
        report = {
            "schema": 1, "purpose": "synthetic_cpu_stack_capability_only_not_acceptance",
            "public_artifacts": True, "attempts": attempts,
            "expected_probe_elf_symbols_verified": symbols_verified,
            "probe_machine_frame_proof": machine_frame_proof,
            "status": attempts[-1]["status"], "canonical_build_allowed": attempts[-1]["status"] == "CAPABLE",
            "helper_sha256": request["helper_sha256"], "probe_sha256": request["target"]["sha256"],
            "probe_build_id": request["target_build_id"], "perf_sha256": request["perf_sha256"],
            "perf_build_id": request["perf_build_id"],
            "perf_version": subprocess.run([perf, "--version"], env=isolated_env(), check=True, capture_output=True, text=True, timeout=bounded_timeout(work_deadline, 2)).stdout.strip(),
        }
    finally:
        if probe is not None and recorder_cleanup_verified:
            stop_owned(probe, None, overall_deadline)
        if recorder_cleanup_verified:
            shutil.rmtree(stage)
    output.parent.mkdir(parents=True, exist_ok=True)
    with output.open("x") as stream:
        json.dump(report, stream, indent=2)
        stream.write("\n")
    return 0 if report["canonical_build_allowed"] else 2


def validate_app_id_index(data, offset, length, misc, attr_offset, attr_size, tids, cpus):
    if misc or length < 16 or not tids or len(set(tids)) != len(tids):
        raise Refused("invalid_application_id_index_scope")
    nr = struct.unpack_from("<Q", data, offset + 8)[0]
    if length != 16 + 32 * nr:
        raise Refused("invalid_or_guest_id_index_length")
    ids = attribute_ids(data, attr_offset, attr_size)
    entries = [struct.unpack_from("<QQQq", data, offset + 16 + 32 * i) for i in range(nr)]
    if nr != len(ids) or {e[0] for e in entries} != set(ids):
        raise Refused("application_id_index_attr_mismatch")
    per_thread = all(e[2] == (1 << 64) - 1 for e in entries)
    cpu_map = [-1] if per_thread else cpus
    expected_pairs = {(cpu % (1 << 64), tid) for cpu in cpu_map for tid in tids}
    if len(entries) != len(expected_pairs) or {(e[2], e[3]) for e in entries} != expected_pairs:
        raise Refused("foreign_or_incomplete_application_id_index")
    for event_id, idx, cpu, tid in entries:
        expected_index = ids.index(event_id) if per_thread else cpus.index(cpu)
        if idx != expected_index:
            raise Refused("application_id_index_ring_mismatch")
        if per_thread and event_id != ids[idx]:
            raise Refused("application_id_index_event_mismatch")


def pinned_process(pid):
    result = process_identity(pid)
    info = os.stat(f"/proc/{pid}/exe")
    result["inode"] = [info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns]
    return result


def fast_same_process(expected):
    try:
        info = os.stat(f"/proc/{expected['pid']}/exe")
        identity = task_identity(expected["pid"])
        return (all(identity[k] == expected[k] for k in identity)
                and str(Path(f"/proc/{expected['pid']}/exe").resolve(strict=True)) == expected["exe"]
                and [info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns] == expected["inode"])
    except (OSError, ValueError):
        return False


def owned_descendants(launch, uid):
    pending, identities = [launch], {}
    while pending:
        pid = pending.pop()
        if pid in identities or len(identities) >= 128:
            raise Refused("invalid_or_unbounded_owned_process_tree")
        identity = task_identity(pid)
        if identity["uid"] != uid or identity["tgid"] != pid:
            raise Refused("foreign_owned_process_tree")
        identities[pid] = identity
        children = set()
        for task in Path(f"/proc/{pid}/task").iterdir():
            children.update(int(value) for value in (task / 'children').read_text().split())
        for child in children:
            if task_identity(child)["ppid"] != pid:
                raise Refused("owned_child_parent_changed")
        pending.extend(sorted(children))
    return identities


def verify_app(request, uid):
    target = request["target"]
    if request.get("helper_sha256") != digest(__file__) or not fast_same_process(target):
        raise Refused("application_or_helper_changed")
    if target["uid"] != uid or target["tgid"] != target["pid"]:
        raise Refused("foreign_application_owner")
    for key in ("runner", "launch"):
        if not fast_same_process(request[key]) or request[key]["uid"] != uid:
            raise Refused("application_ancestry_changed")
    if request["coordinator"]["ppid"] != request["runner"]["pid"] or request["launch"]["ppid"] != request["runner"]["pid"]:
        raise Refused("application_not_owned_by_invoking_runner")
    descendants = owned_descendants(request["launch"]["pid"], uid)
    if target["pid"] not in descendants or digest(f"/proc/{target['pid']}/exe") != target["sha256"]:
        raise Refused("application_not_owned_descendant")
    if build_id(target["exe"], request["work_deadline"]) != request["target_build_id"]:
        raise Refused("application_build_id_changed")
    tids = {int(p.name): task_identity(int(p.name)) for p in Path(f"/proc/{target['pid']}/task").iterdir()}
    if not tids or len(tids) > 256 or any(t["uid"] != uid or t["tgid"] != target["pid"] for t in tids.values()):
        raise Refused("application_threads_outside_bounded_scope")
    if tids != {int(k): v for k, v in request["tids"].items()}:
        raise Refused("application_thread_identity_changed")
    return tids


def own_executable_maps(target):
    info = os.stat(f"/proc/{target['pid']}/exe")
    entries = []
    for line in Path(f"/proc/{target['pid']}/maps").read_text().splitlines():
        fields = line.split(maxsplit=5)
        if len(fields) < 5:
            raise Refused("invalid_process_maps")
        major, minor = (int(v, 16) for v in fields[3].split(':'))
        if int(fields[4]) == info.st_ino and os.makedev(major, minor) == info.st_dev and 'x' in fields[1]:
            begin, end = (int(v, 16) for v in fields[0].split('-'))
            entries.append((begin, end, int(fields[2], 16)))
    if not entries:
        raise Refused("owned_elf_executable_mapping_absent")
    return entries


class OwnElfResolver:
    def __init__(self, maps, loads, symbols, page=4096):
        self.maps, self.symbols = maps, sorted(symbols)
        self.starts = [s[0] for s in self.symbols]
        self.ranges = []
        for begin, end, offset in maps:
            matches = [(vaddr // page * page) + offset - (fileoff // page * page)
                       for fileoff, vaddr, filesz, executable in loads
                       if executable and fileoff // page * page <= offset < ((fileoff + filesz + page - 1) // page * page)]
            if len(matches) != 1:
                raise Refused("ambiguous_owned_elf_load_mapping")
            self.ranges.append((begin, end, begin - matches[0]))

    def __call__(self, ip):
        for begin, end, bias in self.ranges:
            if begin <= ip < end:
                address = ip - bias
                index = bisect.bisect_right(self.starts, address) - 1
                if index >= 0:
                    start, size, name = self.symbols[index]
                    if start <= address < start + size:
                        return name
                return "[own ELF unresolved]"
        return "[external or unmapped]"


def app_symbol_resolver(target, deadline):
    executable = target["exe"]
    def read(*args):
        return subprocess.run(['readelf', *args, executable], check=True, capture_output=True,
                              text=True, timeout=bounded_timeout(deadline, 3), env=isolated_env()).stdout
    symbols = []
    for line in read('--symbols', '--wide').splitlines():
        match = re.match(r'\s*\d+:\s+([0-9a-fA-F]+)\s+(\d+)\s+FUNC\s+\S+\s+\S+\s+(\S+)\s+(\S+)', line)
        if match and match[3] != 'UND' and int(match[2]) > 0:
            # Only names actually defined in the exact verified owned ELF can
            # enter public aggregates. Perf's display names/DSOs are untrusted.
            name = match[4]
            if len(name) <= 1024 and re.fullmatch(r'[A-Za-z0-9_.$@:+<>~,*&()\[\]-]+', name):
                symbols.append((int(match[1], 16), int(match[2]), name))
    loads = []
    for line in read('--program-headers', '--wide').splitlines():
        match = re.match(r'\s*LOAD\s+(0x[0-9a-f]+)\s+(0x[0-9a-f]+)\s+0x[0-9a-f]+\s+(0x[0-9a-f]+)\s+0x[0-9a-f]+\s+(.+?)\s+0x[0-9a-f]+\s*$', line)
        if match:
            loads.append((int(match[1], 16), int(match[2], 16), int(match[3], 16), 'E' in match[4]))
    if not symbols or not loads:
        raise Refused("owned_elf_symbols_or_loads_unavailable_no_codegen_flag_change")
    return OwnElfResolver(own_executable_maps(target), loads, list(set(symbols)), os.sysconf('SC_PAGE_SIZE'))


def aggregate_owned_samples(samples, counts, headers, resolve):
    leaf, inclusive = collections.Counter(), collections.Counter()
    own_samples = caller_samples = 0
    for sample in samples:
        ips = []
        first = re.match(r'cpu-clock:u:\s+([0-9a-fA-F]+)\b', sample[0])
        if first:
            ips.append(int(first[1], 16))
        for line in sample[1:]:
            match = re.match(r'([0-9a-fA-F]+)\s', line)
            if match:
                ips.append(int(match[1], 16))
        if not ips:
            raise Refused("sample_instruction_pointer_not_decoded")
        names = [resolve(ip) for ip in ips]
        leaf[names[0]] += 1
        for name in set(names):
            inclusive[name] += 1
        own_samples += any(not name.startswith('[') for name in names)
        caller_samples += len(ips) > 1
    if len(samples) < 10 or own_samples == 0:
        raise Refused("insufficient_owned_elf_sample_coverage")
    def top(counter):
        return [{'function': name, 'samples': count} for name, count in counter.most_common(20)]
    return {'event_counts': dict(counts), 'validated_header_records': headers,
            'samples': len(samples), 'own_elf_samples': own_samples,
            'samples_with_callers': caller_samples, 'caller_coverage': 'partial_frame_pointer_only',
            'leaf_top20': top(leaf), 'inclusive_top20': top(inclusive)}


def validate_app_manifest(value):
    fields = {'schema', 'source_sha', 'coordinator_pid', 'launch_pid', 'measurement_started_at_utc',
              'measurement_deadline_utc', 'roles', 'case_id', 'execution_mode'}
    if set(value) != fields or value['schema'] != 1 or value['case_id'] != 'sixteen_universe' or value['execution_mode'] != 'unrestricted':
        raise Refused('invalid_owned_application_manifest')
    if not re.fullmatch('[0-9a-f]{40}', value['source_sha']):
        raise Refused('invalid_application_source_pin')
    roles = value['roles']
    if len(roles) != 2 or {r.get('role') for r in roles} != {'server', 'native_renderer'}:
        raise Refused('invalid_application_roles')
    for role in roles:
        if set(role) != {'role', 'sha256', 'build_id'} or not re.fullmatch('[0-9a-f]{64}', role['sha256']) or not re.fullmatch('[0-9a-f]{8,128}', role['build_id']):
            raise Refused('invalid_application_binary_pin')
    start = datetime.datetime.fromisoformat(value['measurement_started_at_utc'].replace('Z', '+00:00'))
    end = datetime.datetime.fromisoformat(value['measurement_deadline_utc'].replace('Z', '+00:00'))
    if start.utcoffset() != datetime.timedelta(0) or end - start != datetime.timedelta(seconds=60):
        raise Refused('invalid_application_measurement_window')
    return end.timestamp()


def collect_owned_requests(requests):
    def collect(request):
        attempts = [collect_attempt(request, False)]
        if attempts[-1].get('code') == 'perf_permission_denied' and attempts[-1].get('recorder_gone') is True:
            attempts.append(collect_attempt(request, True))
        return {'role': request['role'], 'sha256': request['target']['sha256'],
                'build_id': request['target_build_id'], 'attempts': attempts}
    reports, verified = [], True
    with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
        futures = [pool.submit(collect, request) for request in requests]
        # A first failure never discards the other recorder's cleanup receipt.
        for future in futures:
            try:
                report = future.result()
                reports.append(report)
                verified &= all(a.get('recorder_gone') is True for a in report['attempts'])
            except BaseException:
                verified = False
    return reports, verified


def owned_app(manifest, output):
    report = {'schema': 1, 'status': 'STOP', 'recorder_cleanup_verified': True,
              'purpose': 'owned_application_cpu_cause_only_not_performance_acceptance', 'roles': []}
    cleanup_verified = True
    try:
        root = artifact_path('LIGHT_TMP_DIR', 'TMP_ROOT').resolve()
        results = artifact_path('LIGHT_TEST_RESULTS_DIR', 'TEST_RESULTS').resolve()
        if not manifest.is_absolute() or not output.is_absolute() or not any(output.resolve().is_relative_to(p) for p in (root, results)):
            raise Refused('owned_profile_paths_not_canonical')
        if sys.platform != 'linux' or os.getuid() == 0:
            raise Refused('requires_ordinary_linux_ci_coordinator')
        value = json.loads(manifest.read_text())
        utc_deadline = validate_app_manifest(value)
        overall = min(time.monotonic() + 50, time.monotonic() + utc_deadline - time.time() - 5)
        work = overall - 16
        if bounded_timeout(work, 10) < 10:
            raise Refused('insufficient_owned_profile_measurement_budget')
        if value['coordinator_pid'] != os.getppid():
            raise Refused('runner_is_not_actual_invoking_parent')
        runner, launch = pinned_process(os.getppid()), pinned_process(value['launch_pid'])
        if launch['ppid'] != runner['pid'] or launch['uid'] != os.getuid():
            raise Refused('launch_not_owned_runner_child')
        descendants = owned_descendants(launch['pid'], os.getuid())
        perf = find_perf()
        if not perf:
            raise Refused('perf_unavailable')
        base = {'kind': 'owned_app', 'coordinator': pinned_process(os.getpid()), 'runner': runner,
                'launch': launch, 'helper_sha256': digest(__file__), 'spool_parent': str(root),
                'perf': str(Path(perf).resolve()), 'perf_sha256': digest(perf),
                'perf_build_id': build_id(perf, work), 'work_deadline': work, 'overall_deadline': overall}
        requests = []
        for role in value['roles']:
            matches = []
            for pid in descendants:
                name = Path(f'/proc/{pid}/exe').resolve(strict=True).name
                expected_name = 'light-headless' if role['role'] == 'server' else 'viz-renderer'
                if name == expected_name or name.startswith(expected_name + '-'):
                    target = pinned_process(pid)
                    if target['sha256'] == role['sha256'] and build_id(target['exe'], work) == role['build_id']:
                        matches.append(target)
            if len(matches) != 1:
                raise Refused('exact_owned_application_role_not_unique')
            target = matches[0]
            tids = {int(p.name): task_identity(int(p.name)) for p in Path(f"/proc/{target['pid']}/task").iterdir()}
            requests.append({**base, 'role': role['role'], 'target': target, 'target_build_id': role['build_id'],
                             'tids': tids, 'nonce': uuid.uuid4().hex})
        # Validate both ELF resolvers before starting either recorder. No compiler
        # flags or caller-coverage promises are silently introduced here.
        for request in requests:
            verify_app(request, os.getuid())
            app_symbol_resolver(request['target'], work)
        # Mark cleanup unknown before the first asynchronous spawn. A submit or
        # executor failure after a previous submit cannot authorize target teardown.
        cleanup_verified = False
        report['roles'], cleanup_verified = collect_owned_requests(requests)
        report.update(source_sha=value['source_sha'], helper_sha256=digest(__file__),
                      perf_sha256=base['perf_sha256'], perf_build_id=base['perf_build_id'],
                      sample_hz=49, requested_role_seconds=10, global_budget_seconds=50,
                      codegen_flags_changed=False)
        if cleanup_verified and len(report['roles']) == 2 and all(r['attempts'][-1].get('status') == 'CAPABLE' for r in report['roles']):
            report['status'] = 'PROFILED'
        else:
            report['code'] = 'owned_application_profile_refused'
    except (Refused, OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        report['code'] = str(error) if isinstance(error, Refused) else 'owned_profile_scope_or_tools_refused'
    report['recorder_cleanup_verified'] = cleanup_verified
    output.parent.mkdir(parents=True, exist_ok=True)
    with output.open('x') as stream:
        json.dump(report, stream, indent=2)
        stream.write('\n')
    return 0 if report['status'] == 'PROFILED' else 2 if cleanup_verified else 3


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=["preflight", "owned-app", "_record", "_cleanup"])
    parser.add_argument("--output", type=Path)
    parser.add_argument("--manifest", type=Path)
    args = parser.parse_args()
    for signum in (signal.SIGTERM, signal.SIGINT):
        signal.signal(signum, cancelled)
    if args.mode in ("_record", "_cleanup"):
        try:
            (recorder if args.mode == "_record" else recover_recorder)(json.loads(sys.stdin.readline()))
        except (Refused, OSError, ValueError, subprocess.SubprocessError):
            emit({"event": "terminal", "status": "STOP", "code": "helper_scope_or_cleanup_refused"})
            return 2
        return 0
    if args.mode == "owned-app":
        if args.manifest is None or args.output is None:
            parser.error("owned-app requires --manifest and --output")
        return owned_app(args.manifest, args.output)
    if args.output is None:
        parser.error("preflight requires --output under the canonical artifacts tree")
    try:
        return preflight(args.output)
    except (Refused, OSError, ValueError, subprocess.SubprocessError) as error:
        # No raw tool/target metadata is printed on an unvalidated failure.
        failure = {
            "status": "STOP", "canonical_build_allowed": False,
            "code": str(error) if isinstance(error, Refused) else "preflight_unavailable_or_cleanup_unverified",
        }
        if isinstance(error, MachineFrameRefused):
            failure["probe_machine_frame_proof"] = error.diagnostic
        if str(error) != "public_receipt_not_in_canonical_artifacts" and not args.output.exists():
            args.output.parent.mkdir(parents=True, exist_ok=True)
            with args.output.open("x") as stream:
                json.dump(failure, stream)
                stream.write("\n")
        emit(failure)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
