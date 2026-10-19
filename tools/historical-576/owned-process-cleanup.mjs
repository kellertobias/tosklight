// Private cleanup repair draft. No OS operation runs on import.
// Only the caller's freshly spawned detached process group is addressed. No escaped-descendant
// or zombie-reaping claim: success requires direct leader reaping AND actual PGID absence.
import assert from "node:assert/strict";
const GRACE_MILLIS = 2000,
	POLL_MILLIS = 50,
	MAX_POLLS = 128;
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const validPid = (pid) => Number.isSafeInteger(pid) && pid > 0;

export async function stopOwnedGroup(
	child,
	{
		kill = process.kill.bind(process),
		pause = sleep,
		now = () => performance.now(),
	} = {},
) {
	if (!child) return 1;
	const pid = child.pid;
	if (!validPid(pid))
		return Number(
			(child.exitCode !== null && child.exitCode !== undefined) ||
				(child.signalCode !== null && child.signalCode !== undefined),
		);
	let reaped =
		(child.exitCode !== null && child.exitCode !== undefined) ||
		(child.signalCode !== null && child.signalCode !== undefined);
	const onClose = () => {
		reaped = true;
	};
	child.once("close", onClose);
	let last = -Infinity;
	const clock = () => {
		const value = now();
		assert(
			Number.isFinite(value) && value >= 0 && value >= last,
			"invalid monotonic cleanup clock",
		);
		last = value;
		return value;
	};
	const absent = () => {
		try {
			kill(-pid, 0);
			return false;
		} catch (error) {
			if (error.code === "ESRCH") return true;
			throw error;
		}
	};
	try {
		for (const signal of ["SIGTERM", "SIGKILL"]) {
			try {
				kill(-pid, signal);
			} catch (error) {
				if (error.code !== "ESRCH") throw error;
			}
			const deadline = clock() + GRACE_MILLIS;
			// Leader exit never short-circuits this grace. Descendants may still be handling the signal.
			// A persistent zombie group is still present and cannot satisfy the success predicate.
			let expired = false;
			for (let polls = 0; polls < MAX_POLLS; polls++) {
				const groupAbsent = absent();
				if (groupAbsent && reaped) return 1;
				const remaining = deadline - clock();
				if (remaining <= 0) {
					expired = true;
					break;
				}
				await pause(Math.min(POLL_MILLIS, remaining));
			}
			if (!expired) return 0; // An injected/broken clock or wait cannot create an unbounded spin.
		}
		return 0;
	} finally {
		child.removeListener("close", onClose);
	}
}

const pendingByChild = new WeakMap();
// Deliberately non-async: timer, signal and finalizer receive the SAME promise. Unexpected
// permission/clock errors become an unverified cleanup result, never an unhandled signal task.
export function stopOwned(child, dependencies) {
	if (!child) return Promise.resolve(1);
	const previous = pendingByChild.get(child);
	if (previous) return previous;
	const pending = stopOwnedGroup(child, dependencies).catch(() => 0);
	pendingByChild.set(child, pending);
	return pending;
}

// Existing report shape/gates are unchanged. Preserve the first substantive failure frontier.
export function applyCleanupResult(row, verified) {
	assert(verified === 0 || verified === 1);
	row.cleanup_verified = verified;
	if (!verified) {
		row.complete = 0;
		row.failure_stage_code ||= 12;
	}
	return row;
}
