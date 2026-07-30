export function summarizeProgrammerActionTiming(
	records,
	baselineActionId = 0,
	requirements = {},
) {
	const measurements = (Array.isArray(records) ? records : [])
		.filter(
			(record) =>
				Number.isSafeInteger(record?.action_id) &&
				record.action_id > baselineActionId,
		)
		.sort((left, right) => left.action_id - right.action_id);
	const pendingOutput = measurements.filter(
		(record) =>
			record.requires_output_frame === true &&
			record.output_within_budget == null,
	);
	const failed = measurements.filter(
		(record) =>
			record.succeeded !== true ||
			record.acknowledgement_within_budget !== true ||
			(record.requires_output_frame === true &&
				record.output_within_budget !== true),
	);
	const failures = failed.map((record) => {
		const reasons = [];
		if (record.succeeded !== true) reasons.push("action failed");
		if (record.acknowledgement_within_budget !== true)
			reasons.push("ack exceeded tick or wall budget");
		if (
			record.requires_output_frame === true &&
			record.output_within_budget == null
		)
			reasons.push("first output frame is pending");
		else if (
			record.requires_output_frame === true &&
			record.output_within_budget !== true
		)
			reasons.push("first output frame exceeded tick or wall budget");
		return `${record.source}/${record.action} #${record.action_id}: ${reasons.join(", ")}`;
	});
	const minimumSamples = requirements.minimumSamples ?? 1;
	if (measurements.length < minimumSamples)
		failures.push(
			`recorded ${measurements.length} programmer actions; required at least ${minimumSamples}`,
		);
	for (const source of requirements.sources ?? [])
		if (!measurements.some((record) => record.source === source))
			failures.push(`missing required action source ${source}`);
	for (const action of requirements.actions ?? [])
		if (!measurements.some((record) => record.action === action))
			failures.push(`missing required action ${action}`);
	for (const band of requirements.frameRateBands ?? []) {
		const present = measurements.some((record) =>
			band === "at-or-below-60"
				? record.output_frame_hz <= 60 && record.budget_ticks === 2
				: record.output_frame_hz > 60 && record.budget_ticks === 4,
		);
		if (!present) failures.push(`missing required frame-rate band ${band}`);
	}
	return {
		baselineActionId,
		samples: measurements.length,
		measurements,
		pendingOutput: pendingOutput.length,
		failed: failed.length,
		passed: failures.length === 0,
		failures,
	};
}

export function latestProgrammerActionId(records) {
	return Math.max(
		0,
		...(Array.isArray(records) ? records : []).map((record) =>
			Number.isSafeInteger(record?.action_id) ? record.action_id : 0,
		),
	);
}
