const COVERAGE_TOLERANCE_MILLIS = 5_000;

export function summarizeStageLongRunResources(
	timeline,
	processMemory,
	durationSeconds,
	{ renderer = "webgl", stageWindow = null } = {},
) {
	const enforced = durationSeconds >= 1_800;
	const native = renderer === "native";
	const intendedWindow = native ? validStageWindow(stageWindow, durationSeconds) : null;
	const stageMemory = processMemory.filter(
		(sample) => sample.phase === "stage" && (!native ||
			(Number.isFinite(sample.residentBytes) && Number.isFinite(sample.recordedAt) &&
				(!intendedWindow || inStageWindow(sample.recordedAt, intendedWindow)))),
	);
	if (native) stageMemory.sort((left, right) => left.recordedAt - right.recordedAt);
	const warmupOrigin = intendedWindow?.startedAt ?? stageMemory[0]?.recordedAt;
	const memoryAfterWarmup =
		stageMemory.length === 0
			? []
			: stageMemory.filter(
					(sample) => sample.recordedAt >= warmupOrigin + 60_000,
				);
	const slopeBytesPerMinute = linearSlopePerMinute(
		memoryAfterWarmup.map((sample) => [
			sample.recordedAt,
			sample.residentBytes,
		]),
	);
	const expanded = expandRenderSamples(timeline, native);
	const rendered = native ? expanded.filter((sample) => Number.isFinite(sample.recordedAt) &&
		(!intendedWindow || inStageWindow(sample.recordedAt, intendedWindow))) : expanded;
	if (native) rendered.sort((left, right) => left.recordedAt - right.recordedAt);
	const firstRenderAt = intendedWindow?.startedAt ?? rendered[0]?.recordedAt ?? null;
	const early =
		firstRenderAt === null
			? []
			: rendered.filter(
					(sample) =>
						sample.recordedAt >= firstRenderAt + 60_000 &&
						sample.recordedAt < firstRenderAt + 360_000,
				);
	const finalRecordedAt = intendedWindow?.completedAt ?? rendered.at(-1)?.recordedAt ?? null;
	const late =
		finalRecordedAt === null
			? []
			: rendered.filter(
					(sample) => sample.recordedAt >= finalRecordedAt - 300_000,
				);
	const earlyResources = renderWindowResources(early);
	const lateResources = renderWindowResources(late);
	const failures = [];
	const coverage = native ? {
		stageWindow: intendedWindow,
		endpointToleranceMillis: COVERAGE_TOLERANCE_MILLIS,
		durationToleranceMillis: COVERAGE_TOLERANCE_MILLIS,
		requestedDurationMillis: durationSeconds * 1_000,
		actualDurationMillis: Number.isFinite(stageWindow?.startedAt) && Number.isFinite(stageWindow?.completedAt)
			? stageWindow.completedAt - stageWindow.startedAt : null,
		rss: endpointCoverage(stageMemory, intendedWindow),
		renders: endpointCoverage(rendered, intendedWindow),
		panes: [...new Set(rendered.map((sample) => sample.latestRender.paneId).filter((id) => typeof id === "string" && id.length > 0))]
			.map((paneId) => ({ paneId, ...endpointCoverage(rendered.filter((sample) => sample.latestRender.paneId === paneId), intendedWindow) })),
	} : null;
	if (enforced && native) {
		if (!intendedWindow) failures.push("30-minute native run requires an independent complete Stage window");
		if (!coverage.rss.complete) failures.push("30-minute native Stage RSS coverage does not span the intended window without gaps above 5 seconds");
		if (!coverage.renders.complete) failures.push("30-minute native Stage render coverage does not span the intended window without gaps above 5 seconds");
		if (rendered.some((sample) => typeof sample.latestRender.paneId !== "string" || !sample.latestRender.paneId))
			failures.push("30-minute native Stage render pane identity is unavailable");
		for (const pane of coverage.panes)
			if (!pane.complete) failures.push(`30-minute native Stage render coverage is incomplete for pane ${pane.paneId}`);
	}
	if (enforced && memoryAfterWarmup.length < 60)
		failures.push(
			"30-minute packaged run did not retain enough main-process memory samples",
		);
	if (
		enforced &&
		(slopeBytesPerMinute === null || slopeBytesPerMinute > 1_048_576)
	)
		failures.push(
			"30-minute packaged main-process memory grew faster than 1 MiB per minute after warmup",
		);
	if (enforced && (early.length === 0 || late.length === 0))
		failures.push(
			"30-minute packaged run has no comparable early and late render windows",
		);
	if (
		enforced &&
		(lateResources.maxGeometries > earlyResources.maxGeometries ||
			(renderer === "native"
				? lateResources.maxDrawCalls > earlyResources.maxDrawCalls
				: lateResources.maxTextures > earlyResources.maxTextures))
	)
		failures.push(
			renderer === "native"
				? "30-minute packaged native instance or draw-call ownership grew after warmup"
				: "30-minute packaged WebGL geometry or texture counts grew after warmup",
		);
	if (enforced && !Number.isFinite(lateResources.cpuFrameP95Ms))
		failures.push("30-minute packaged late-window CPU submission p95 is unavailable");
	if (enforced && lateResources.cpuFrameP95Ms > 16.7)
		failures.push(
			"30-minute packaged late-window CPU submission p95 exceeded 16.7 ms",
		);
	return {
		enforced,
		memoryWarmupSeconds: 60,
		maximumMemorySlopeBytesPerMinute: 1_048_576,
		slopeBytesPerMinute,
		memorySamplesAfterWarmup: memoryAfterWarmup.length,
		...(native ? { coverage } : {}),
		early: earlyResources,
		late: lateResources,
		failures,
		passed: enforced ? failures.length === 0 : null,
		gpuFrameMeasurement:
			renderer === "native"
				? "Native helper frame telemetry; aggregate p95 is reported in resources.gpuFrameP95Ms"
				: "Unavailable in the current Three.js/WebKit diagnostics path",
	};
}

function validStageWindow(window, durationSeconds) {
	if (!Number.isFinite(window?.startedAt) || !Number.isFinite(window?.completedAt) ||
		window.completedAt <= window.startedAt || window.completedAt - window.startedAt < durationSeconds * 1_000 - COVERAGE_TOLERANCE_MILLIS) return null;
	return { startedAt: window.startedAt, completedAt: window.completedAt };
}

function inStageWindow(timestamp, window) {
	return window !== null && Number.isFinite(timestamp) && timestamp >= window.startedAt && timestamp <= window.completedAt;
}

function endpointCoverage(samples, window) {
	const firstRecordedAt = samples[0]?.recordedAt ?? null;
	const lastRecordedAt = samples.at(-1)?.recordedAt ?? null;
	const maximumGapMillis = samples.reduce((largest, sample, index) => index === 0 ? largest
		: Math.max(largest, sample.recordedAt - samples[index - 1].recordedAt), 0);
	return { firstRecordedAt, lastRecordedAt, maximumGapMillis,
		complete: window !== null && firstRecordedAt !== null && lastRecordedAt !== null &&
			firstRecordedAt <= window.startedAt + COVERAGE_TOLERANCE_MILLIS && lastRecordedAt >= window.completedAt - COVERAGE_TOLERANCE_MILLIS &&
			maximumGapMillis <= COVERAGE_TOLERANCE_MILLIS };
}

function expandRenderSamples(timeline, native) {
	const entry = (sample, render) => ({
		...sample, latestRender: render,
		// Native submission clocks are epoch-based. Observation time cannot refresh an old image.
		...(native ? { recordedAt: Number.isFinite(render.recordedAt) ? render.recordedAt : render.submittedAt } : {}),
	});
	const exact = timeline.flatMap((sample) => Array.isArray(sample.newRenders)
		? sample.newRenders.map((render) => entry(sample, render)) : []);
	if (exact.length > 0 || (native && timeline.some((sample) => Array.isArray(sample.newRenders)))) return exact;
	const deduplicated = new Map();
	for (const sample of timeline) {
		if (!sample.latestRender) continue;
		const identity = sample.latestRender.benchmarkSequence ??
			`${sample.latestRender.paneId}:${sample.latestRender.submittedAt}:${sample.latestRender.renderQuality}`;
		if (!native || !deduplicated.has(identity)) deduplicated.set(identity, entry(sample, sample.latestRender));
	}
	return [...deduplicated.values()];
}

function renderWindowResources(samples) {
	const renders = samples.map((sample) => sample.latestRender);
	return {
		samples: renders.length,
		maxGeometries: maximumField(renders, "geometries"),
		maxTextures: maximumField(renders, "textures"),
		maxDrawCalls: maximumField(renders, "calls"),
		maxTransparentDrawCalls: maximumField(renders, "transparentDrawCalls"),
		cpuFrameP95Ms: percentile(
			renders
				.map((render) => render.durationMs)
				.filter(Number.isFinite)
				.sort((left, right) => left - right),
			95,
		),
	};
}

function maximumField(values, field) {
	return Math.max(0, ...values.map((value) => Number(value[field] ?? 0)));
}

function linearSlopePerMinute(samples) {
	if (samples.length < 2) return null;
	const origin = samples[0][0];
	const points = samples.map(([timestamp, value]) => [
		(timestamp - origin) / 60_000,
		value,
	]);
	const meanX = points.reduce((sum, [x]) => sum + x, 0) / points.length;
	const meanY = points.reduce((sum, [, y]) => sum + y, 0) / points.length;
	const denominator = points.reduce((sum, [x]) => sum + (x - meanX) ** 2, 0);
	if (denominator === 0) return null;
	return (
		points.reduce((sum, [x, y]) => sum + (x - meanX) * (y - meanY), 0) /
		denominator
	);
}

function percentile(sorted, value) {
	if (!sorted.length) return null;
	return sorted[Math.max(0, Math.ceil((value / 100) * sorted.length) - 1)];
}
