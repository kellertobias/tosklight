export function changingPresentationGaps(
	frames,
	applicationSuspend,
	showSwitch,
	contextRecovery,
) {
	const groups = new Map();
	for (const frame of frames) {
		if (
			frameOverlapsApplicationSuspend(frame, applicationSuspend) ||
			frameOverlapsShowSwitch(frame, showSwitch) ||
			frameOverlapsContextRecovery(frame, contextRecovery)
		)
			continue;
		const key = `${frame.lane}:${frame.showId ?? "unknown"}:${frame.scopeActivation ?? 0}:${frame.claimActivation ?? 0}:${frame.visibilitySegment}:${lifecycleSegment(frame, applicationSuspend, showSwitch, contextRecovery)}`;
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

export function laneSourceCadenceGaps(
	frames,
	applicationSuspend,
	showSwitch,
	contextRecovery,
) {
	const lanes = new Map();
	for (const frame of frames) {
		if (
			frameOverlapsApplicationSuspend(frame, applicationSuspend) ||
			frameOverlapsShowSwitch(frame, showSwitch) ||
			frameOverlapsContextRecovery(frame, contextRecovery)
		)
			continue;
		const sourceAt = frameSourceAt(frame);
		if (!Number.isFinite(sourceAt)) continue;
		const laneKey = `${frame.lane}:${frame.showId ?? "unknown"}:${frame.scopeActivation ?? 0}:${frame.claimActivation ?? 0}:${lifecycleSegment(frame, applicationSuspend, showSwitch, contextRecovery)}`;
		const lane = lanes.get(laneKey) ?? [];
		lane.push({ sourceAt, visibleChanged: frame.visibleChanged === true });
		lanes.set(laneKey, lane);
	}
	return [...lanes.values()].flatMap((values) => {
		const ordered = values.sort(
			(left, right) => left.sourceAt - right.sourceAt,
		);
		const gaps = [];
		let previousChangingSourceAt = null;
		for (const value of ordered) {
			if (!value.visibleChanged) {
				previousChangingSourceAt = null;
				continue;
			}
			if (
				Number.isFinite(previousChangingSourceAt) &&
				value.sourceAt !== previousChangingSourceAt
			)
				gaps.push(value.sourceAt - previousChangingSourceAt);
			previousChangingSourceAt = value.sourceAt;
		}
		return gaps;
	});
}

export function frameOverlapsApplicationSuspend(frame, applicationSuspend) {
	const suspendedAt = Date.parse(applicationSuspend?.startedAt);
	const resumedAt = Date.parse(applicationSuspend?.finishedAt);
	if (!Number.isFinite(suspendedAt) || !Number.isFinite(resumedAt)) return false;
	const times = [
		frameSourceAt(frame),
		frame.receivedAt,
		frame.firstCanvasSubmittedAt,
		frame.settledCanvasSubmittedAt,
	].filter(Number.isFinite);
	if (!times.length) return false;
	return Math.min(...times) <= resumedAt && Math.max(...times) >= suspendedAt;
}

export function frameOverlapsShowSwitch(frame, showSwitch) {
	return (showSwitch?.intervals ?? []).some((interval) =>
		frameOverlapsInterval(frame, interval),
	);
}

export function frameOverlapsContextRecovery(frame, contextRecovery) {
	return frameOverlapsInterval(frame, contextRecovery);
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

function lifecycleSegment(
	frame,
	applicationSuspend,
	showSwitch,
	contextRecovery,
) {
	const resumedAt = Date.parse(applicationSuspend?.finishedAt);
	const sourceAt = frameSourceAt(frame);
	const suspendSegment =
		Number.isFinite(resumedAt) && sourceAt >= resumedAt ? 1 : 0;
	const showSwitchSegment = (showSwitch?.intervals ?? []).filter((interval) => {
		const finishedAt = Date.parse(interval.finishedAt);
		return Number.isFinite(finishedAt) && sourceAt >= finishedAt;
	}).length;
	const recoveryFinishedAt = Date.parse(contextRecovery?.finishedAt);
	const recoverySegment =
		Number.isFinite(recoveryFinishedAt) && sourceAt >= recoveryFinishedAt ? 1 : 0;
	return `${suspendSegment}:${showSwitchSegment}:${recoverySegment}`;
}

function frameOverlapsInterval(frame, interval) {
	const startedAt = Date.parse(interval?.startedAt);
	const finishedAt = Date.parse(interval?.finishedAt);
	if (!Number.isFinite(startedAt) || !Number.isFinite(finishedAt)) return false;
	const times = [
		frameSourceAt(frame),
		frame.receivedAt,
		frame.firstCanvasSubmittedAt,
		frame.settledCanvasSubmittedAt,
	].filter(Number.isFinite);
	if (!times.length) return false;
	return Math.min(...times) <= finishedAt && Math.max(...times) >= startedAt;
}

function frameSourceAt(frame) {
	const parsed = Date.parse(frame.sourceGeneratedAt);
	return Number.isFinite(parsed) ? parsed : frame.receivedAt;
}
