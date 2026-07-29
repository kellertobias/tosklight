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
