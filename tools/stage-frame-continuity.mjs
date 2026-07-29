export function changingPresentationGaps(frames, applicationSuspend) {
	const groups = new Map();
	for (const frame of frames) {
		const key = `${frame.lane}:${frame.showId ?? "unknown"}:${frame.scopeActivation ?? 0}:${frame.visibilitySegment}:${lifecycleSegment(frame, applicationSuspend)}`;
		const group = groups.get(key) ?? [];
		group.push(frame);
		groups.set(key, group);
	}
	const gaps = [];
	for (const group of groups.values()) {
		let previousSourceAt = null;
		let previousCanvasAt = null;
		for (const frame of group.sort(
			(left, right) => frameSourceAt(left) - frameSourceAt(right),
		)) {
			const sourceAt = frameSourceAt(frame);
			const continuous =
				frame.visibleChanged === true &&
				Number.isFinite(sourceAt) &&
				Number.isFinite(previousSourceAt) &&
				sourceAt - previousSourceAt <= 200;
			if (!continuous) previousCanvasAt = null;
			const canvasAt =
				frame.firstCanvasSubmittedAt ?? frame.settledCanvasSubmittedAt;
			if (frame.visibleChanged === true && Number.isFinite(canvasAt)) {
				if (continuous && Number.isFinite(previousCanvasAt))
					gaps.push(canvasAt - previousCanvasAt);
				previousCanvasAt = canvasAt;
			}
			previousSourceAt =
				frame.visibleChanged === true && Number.isFinite(sourceAt)
					? sourceAt
					: null;
		}
	}
	return gaps;
}

export function laneSourceCadenceGaps(frames, applicationSuspend) {
	const lanes = new Map();
	for (const frame of frames) {
		const sourceAt = frameSourceAt(frame);
		if (!Number.isFinite(sourceAt)) continue;
		const laneKey = `${frame.lane}:${frame.showId ?? "unknown"}:${frame.scopeActivation ?? 0}:${lifecycleSegment(frame, applicationSuspend)}`;
		const lane = lanes.get(laneKey) ?? [];
		lane.push(sourceAt);
		lanes.set(laneKey, lane);
	}
	return [...lanes.values()].flatMap((values) => {
		const ordered = [...new Set(values)].sort((left, right) => left - right);
		const gaps = [];
		for (let index = 1; index < ordered.length; index++)
			gaps.push(ordered[index] - ordered[index - 1]);
		return gaps;
	});
}

export function latestChangingFrameDidNotSettle(frames, completedAt) {
	const completionTime = Date.parse(completedAt);
	const latestByLane = new Map();
	for (const frame of frames) {
		const sourceAt = frameSourceAt(frame);
		const previous = latestByLane.get(frame.lane);
		if (!previous || sourceAt > frameSourceAt(previous))
			latestByLane.set(frame.lane, frame);
	}
	return [...latestByLane.values()].some(
		(frame) =>
			frame.visibleChanged === true &&
			!Number.isFinite(frame.settledCanvasSubmittedAt) &&
			Number.isFinite(completionTime) &&
			completionTime - frame.receivedAt > 200,
	);
}

function lifecycleSegment(frame, applicationSuspend) {
	const resumedAt = Date.parse(applicationSuspend?.finishedAt);
	if (!Number.isFinite(resumedAt)) return 0;
	return frameSourceAt(frame) >= resumedAt ? 1 : 0;
}

function frameSourceAt(frame) {
	const parsed = Date.parse(frame.sourceGeneratedAt);
	return Number.isFinite(parsed) ? parsed : frame.receivedAt;
}
