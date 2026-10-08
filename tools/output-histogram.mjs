export function outputWindow(before, after) {
	return {
		frames_sent: after.frames_sent - before.frames_sent,
		packets_sent: after.packets_sent - before.packets_sent,
		send_errors: after.send_errors - before.send_errors,
		deadline_misses: after.deadline_misses - before.deadline_misses,
		maximum_lateness_micros: after.maximum_lateness_micros,
		last_tick_micros: after.last_tick_micros,
		maximum_tick_micros: after.maximum_tick_micros,
		scheduler_utilization: after.scheduler_utilization,
		tick_duration_bucket_bounds_micros: [
			...after.tick_duration_bucket_bounds_micros,
		],
		tick_duration_bucket_counts: after.tick_duration_bucket_counts.map(
			(count, index) =>
				Math.max(0, count - (before.tick_duration_bucket_counts[index] ?? 0)),
		),
	};
}

export function histogramPercentileMicros(window, percentile) {
	const samples = window.tick_duration_bucket_counts.reduce(
		(total, count) => total + count,
		0,
	);
	if (samples === 0) return null;
	const rank = Math.ceil((percentile / 100) * samples);
	let cumulative = 0;
	for (
		let index = 0;
		index < window.tick_duration_bucket_counts.length;
		index++
	) {
		cumulative += window.tick_duration_bucket_counts[index] ?? 0;
		if (cumulative >= rank)
			return window.tick_duration_bucket_bounds_micros[index] ?? null;
	}
	return null;
}

/**
 * Accumulates an output window across scheduler counter resets. Opening a show restarts the
 * output scheduler, so its cumulative counters start again from zero; subtracting the window's
 * first snapshot from its last would then lose or clamp every tick before the reset. Observing
 * the counters often enough and adding the per-interval differences keeps every tick except the
 * few between the last observation before a reset and the reset itself.
 */
export function createOutputWindowAccumulator(start) {
	let previous = start;
	let total = emptyWindow(start);
	let resets = 0;
	let lateIntervals = [];
	const observe = (current, observedAt = Date.now()) => {
		const reset = isCounterReset(previous, current);
		if (reset) resets++;
		const interval = outputWindow(
			reset ? zeroSnapshot(current) : previous,
			current,
		);
		// Where the window's misses happened, so they can be set against its lifecycle events.
		if (interval.deadline_misses > 0 || interval.send_errors > 0)
			lateIntervals.push({
				observedAt: new Date(observedAt).toISOString(),
				deadline_misses: interval.deadline_misses,
				send_errors: interval.send_errors,
				counter_reset: reset,
			});
		total = addWindows(total, interval);
		previous = current;
	};
	return {
		observe,
		take(current) {
			observe(current);
			const window = {
				...total,
				counter_resets: resets,
				late_intervals: lateIntervals,
			};
			total = emptyWindow(current);
			resets = 0;
			lateIntervals = [];
			return window;
		},
	};
}

function isCounterReset(previous, current) {
	return (
		current.frames_sent < previous.frames_sent ||
		current.tick_duration_bucket_counts.some(
			(count, index) =>
				count < (previous.tick_duration_bucket_counts[index] ?? 0),
		)
	);
}

function zeroSnapshot(snapshot) {
	return {
		...snapshot,
		frames_sent: 0,
		packets_sent: 0,
		send_errors: 0,
		deadline_misses: 0,
		tick_duration_bucket_counts: snapshot.tick_duration_bucket_counts.map(
			() => 0,
		),
	};
}

function emptyWindow(snapshot) {
	return {
		...outputWindow(snapshot, snapshot),
		maximum_lateness_micros: 0,
		maximum_tick_micros: 0,
	};
}

function addWindows(total, next) {
	return {
		frames_sent: total.frames_sent + next.frames_sent,
		packets_sent: total.packets_sent + next.packets_sent,
		send_errors: total.send_errors + next.send_errors,
		deadline_misses: total.deadline_misses + next.deadline_misses,
		maximum_lateness_micros: Math.max(
			total.maximum_lateness_micros,
			next.maximum_lateness_micros,
		),
		last_tick_micros: next.last_tick_micros,
		maximum_tick_micros: Math.max(
			total.maximum_tick_micros,
			next.maximum_tick_micros,
		),
		scheduler_utilization: next.scheduler_utilization,
		tick_duration_bucket_bounds_micros: [
			...next.tick_duration_bucket_bounds_micros,
		],
		tick_duration_bucket_counts: next.tick_duration_bucket_counts.map(
			(count, index) => count + (total.tick_duration_bucket_counts[index] ?? 0),
		),
	};
}
