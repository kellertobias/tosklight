"""Opt-in Linux perf capability preflight; never runs a ToskLight workload.

No invocation of perf is authorized merely by importing or testing this module.
The public receipt contains only validated synthetic-probe evidence. Unvalidated
perf output stays in a no-follow, private spool and is never printed or uploaded.
"""

from __future__ import annotations

import argparse
import collections
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
    for (unsigned i = 0; i < 100000; ++i) n = n * 1664525 + 1013904223;
    return n;
}
__attribute__((noinline)) void tl775_profile_probe_parent(void) {
    while (!done) answer = tl775_profile_probe_leaf(answer);
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


class CensusRefused(Refused):
    """Numeric record-header diagnostics only; no event payload is published."""

    def __init__(self, event, misc, length, counts, sample_type, flags):
        super().__init__("unsupported_or_lost_perf_record")
        self.diagnostic = {
            "record_type": event, "record_misc": misc, "record_length": length,
            "preceding_record_counts": dict(counts),
            "attribute_sample_type": sample_type, "attribute_flags": flags,
        }


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
        return process_identity(expected["pid"]) == expected
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
            if parent != coordinator or parent["uid"] != uid:
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
        "--no-inherit", "--no-buildid-cache", "--pid", str(pid),
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


def event_census(data):
    """Read only file/record headers; installed perf decodes every event payload.

    PERFILE2 header/record layout: Linux tools/perf/util/header.h and
    include/uapi/linux/perf_event.h. No custom stack/sample payload decoder.
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
    while offset < end:
        if offset + 8 > end:
            raise Refused("truncated_perf_record")
        event, misc, length = struct.unpack_from("<IHH", data, offset)
        if length < 8 or offset + length > end:
            raise Refused("invalid_perf_record_length")
        if event not in names:
            raise CensusRefused(event, misc, length, counts, sample_type, flags)
        if event in (1, 9, 10) and misc & 7 != 2:
            raise Refused("non_user_sample_or_mapping")
        counts[names[event]] += 1
        offset += length
    return counts


def validate_export(text, census, target, tids, interval):
    """Strict installed-perf typed output; unknown syntax or missing records refuses.

    SAMPLE prefix is selected with -F pid,tid,time,event,ip,sym,dso. Metadata
    spellings follow Linux tools/perf/util/event.c; no unknown line is ignored.
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
            raise Refused("unsupported_typed_event_syntax")
        pid, tid = int(match[1]), int(match[2])
        if pid != target["pid"] or tid not in tids:
            raise Refused("foreign_or_unverified_event_identity")
        body = match[4]
        seconds, nanos = match[3].split(".")
        if len(nanos) > 9:
            raise Refused("unsupported_timestamp_precision")
        timestamp = int(seconds) * 1_000_000_000 + int(nanos.ljust(9, "0"))
        initial_metadata = timestamp == 0 and body.startswith(("PERF_RECORD_COMM", "PERF_RECORD_MMAP"))
        if not initial_metadata and not interval[0] <= timestamp <= interval[1]:
            raise Refused("event_outside_verified_target_lifetime")
        if body.startswith("cpu-clock:u:"):
            counts["SAMPLE"] += 1
            current = [body]
            samples.append(current)
            continue
        current = None
        event = re.match(r"PERF_RECORD_(COMM|MMAP2?|FORK|EXIT)\b(.*)", body)
        if not event:
            raise Refused("unsupported_typed_metadata")
        kind, detail = event.groups()
        separator = ":" if kind in ("FORK", "EXIT") else "/"
        identities = re.findall(rf"(?<!\d)(\d+)\s*{separator}\s*(\d+)(?!\d)", detail)
        if not identities:
            raise Refused("metadata_identity_not_decoded")
        for owner, thread in identities:
            if int(owner) != target["pid"] or int(thread) not in tids:
                raise Refused("foreign_metadata_identity")
        counts[kind] += 1
    if counts != census:
        raise Refused("typed_event_census_mismatch")
    useful = sum(all(any(name in frame for frame in sample) for name in PROBE_NAMES) for sample in samples)
    if len(samples) < 10 or useful < 10:
        raise Refused("symbolized_probe_callchain_unavailable")
    return {"event_counts": dict(counts), "samples": len(samples), "probe_chains": useful}


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
    tids = verify_probe(request, uid)
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
        deadline = time.monotonic() + 2
        if deadline > request["work_deadline"]:
            raise Refused("insufficient_budget_for_two_second_probe")
        while time.monotonic() < deadline:
            if not same_process(request["target"]) or select.select([target_fd], [], [], 0)[0]:
                raise Refused("target_died_or_changed_quarantined")
            if not same_process(request["coordinator"]):
                raise Refused("coordinator_changed_quarantined")
            for tid, expected in tids.items():
                if task_identity(tid) != expected:
                    raise Refused("thread_lifetime_changed_quarantined")
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
        raw_fd = os.open("perf.data", os.O_RDONLY | os.O_NOFOLLOW, dir_fd=fd)
        try:
            info = os.fstat(raw_fd)
            if not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid() or info.st_nlink != 1 or info.st_size > LIMIT:
                raise Refused("unexpected_perf_output_inode")
            raw = os.read(raw_fd, LIMIT + 1)
            census = event_census(raw)
            typed = run_perf_tool(perf, [
                "script", "--ns", "--show-task-events", "--show-mmap-events",
                "-F", "pid,tid,time,event,ip,sym,dso", "-i", "perf.data",
            ], fd, request["work_deadline"])
            result = validate_export(typed, census, request["target"], tids, (begin, end))
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
            return {"status": "STOP", "code": "helper_aborted_recorder_recovered"}
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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=["preflight", "_record", "_cleanup"])
    parser.add_argument("--output", type=Path)
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
        if str(error) != "public_receipt_not_in_canonical_artifacts" and not args.output.exists():
            args.output.parent.mkdir(parents=True, exist_ok=True)
            with args.output.open("x") as stream:
                json.dump(failure, stream)
                stream.write("\n")
        emit(failure)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
