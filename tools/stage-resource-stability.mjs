export function summarizeStageLongRunResources(
	timeline,
	processMemory,
	durationSeconds,
) {
	const enforced = durationSeconds >= 1_800;
	const stageMemory = processMemory.filter(
		(sample) => sample.phase === "stage",
	);
	const memoryAfterWarmup =
		stageMemory.length === 0
			? []
			: stageMemory.filter(
					(sample) => sample.recordedAt >= stageMemory[0].recordedAt + 60_000,
				);
	const slopeBytesPerMinute = linearSlopePerMinute(
		memoryAfterWarmup.map((sample) => [
			sample.recordedAt,
			sample.residentBytes,
		]),
	);
	const rendered = expandRenderSamples(timeline);
	const firstRenderAt = rendered[0]?.recordedAt ?? null;
	const early =
		firstRenderAt === null
			? []
			: rendered.filter(
					(sample) =>
						sample.recordedAt >= firstRenderAt + 60_000 &&
						sample.recordedAt < firstRenderAt + 360_000,
				);
	const finalRecordedAt = rendered.at(-1)?.recordedAt ?? null;
	const late =
		finalRecordedAt === null
			? []
			: rendered.filter(
					(sample) => sample.recordedAt >= finalRecordedAt - 300_000,
				);
	const earlyResources = renderWindowResources(early);
	const lateResources = renderWindowResources(late);
	const failures = [];
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
			lateResources.maxTextures > earlyResources.maxTextures)
	)
		failures.push(
			"30-minute packaged WebGL geometry or texture counts grew after warmup",
		);
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
		early: earlyResources,
		late: lateResources,
		failures,
		passed: enforced ? failures.length === 0 : null,
		gpuFrameMeasurement:
			"Unavailable in the current Three.js/WebKit diagnostics path",
	};
}

function expandRenderSamples(timeline) {
	const exact = timeline.flatMap((sample) =>
		Array.isArray(sample.newRenders)
			? sample.newRenders.map((render) => ({
					...sample,
					latestRender: render,
				}))
			: [],
	);
	if (exact.length > 0) return exact;
	const deduplicated = new Map();
	for (const sample of timeline) {
		if (!sample.latestRender) continue;
		const identity =
			sample.latestRender.benchmarkSequence ??
			`${sample.latestRender.paneId}:${sample.latestRender.submittedAt}:${sample.latestRender.renderQuality}`;
		deduplicated.set(identity, sample);
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
