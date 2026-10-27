export function outputWindow(before, after) {
	return {
        ...fineWindow(before, after),
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
export function createOutputWindowAccumulator(start, options = {}) {
	const diagnostic = options.diagnosticWindows
		? createDiagnosticWindows(start, options.observedAt, options.maximumDiagnosticWindows ?? 256)
		: null;
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
				observedAt: validObservationClock(observedAt) ? new Date(observedAt).toISOString() : null,
				...(!validObservationClock(observedAt) ? { clockAvailability: "unavailable" } : {}),
				deadline_misses: interval.deadline_misses,
				send_errors: interval.send_errors,
				counter_reset: reset,
			});
		total = addWindows(total, interval);
		diagnostic?.observe(interval, reset, observedAt);
		previous = current;
	};
	return {
		observe,
		observationFailed() { diagnostic?.observationFailed(); },
		take(current, observedAt = Date.now()) {
			observe(current, observedAt);
			const window = {
				...total,
				counter_resets: resets,
				late_intervals: lateIntervals,
				...(diagnostic ? { diagnostic_windows: diagnostic.take(current, observedAt) } : {}),
			};
			total = emptyWindow(current);
			resets = 0;
			lateIntervals = [];
			return window;
		},
	};
}

/** Diagnostics only: assign whole sampled deltas to receipt-bounded windows, never apportion
 * a delta across a nominal minute. Cumulative maxima are deliberately omitted from each row.
 * Neither these rows nor their coverage status participates in the aggregate acceptance gate. */
function createDiagnosticWindows(snapshot, observedAt, maximumWindows) {
	if (!Number.isSafeInteger(maximumWindows) || maximumWindows < 1 || maximumWindows > 256)
		throw new RangeError("Output diagnostic window capacity must be between 1 and 256");
	let rows = [], overflow = 0, firstOverflow = null;
	let row = diagnosticRow(snapshot, observedAt);
	let previousAt = validObservationClock(observedAt) ? observedAt : null;
	const close = (finalPartial) => {
		const saved = finishDiagnosticRow(row, finalPartial);
		if (rows.length < maximumWindows) rows.push(saved);
		else {
			overflow++;
			firstOverflow ??= saved.startedAtEpochMillis;
		}
	};
	return {
		observationFailed() { row.samplingErrors++; },
		observe(interval, reset, at) {
			if (!sameCoarseGeometry(row.output, interval)) row.coarseGeometryDriftIntervals++;
			row.output = addWindows(row.output, interval);
			row.sampleIntervals++;
			row.counterResets += Number(reset);
			if (!validObservationClock(at) || previousAt === null || at < previousAt) {
				row.invalidClockIntervals++;
			} else {
				const gap = at - previousAt;
				row.maximumObservationGapMillis = Math.max(row.maximumObservationGapMillis, gap);
				if (gap > 5_000) row.gapsAboveFiveSeconds++;
			}
			row.endedAtEpochMillis = validObservationClock(at) ? at : null;
			previousAt = validObservationClock(at) ? at : null;
			if (row.startedAtEpochMillis !== null && row.endedAtEpochMillis !== null
				&& row.endedAtEpochMillis - row.startedAtEpochMillis >= 60_000) {
				close(false);
				row = diagnosticRow(interval, at);
			}
		},
		take(current, at) {
			if (row.sampleIntervals > 0 || row.samplingErrors > 0) close(true);
			const report = {
				schemaVersion: 1, diagnosticOnly: true, targetWindowMillis: 60_000,
				maximumWindows, overflowWindows: overflow, firstOverflowEpochMillis: firstOverflow,
				coverageComplete: overflow === 0 && rows.every((item) => item.coverage.status === "complete"),
				clock: "Client wall-clock receipt after each counter snapshot; not engine tick timestamps or exact HTTP snapshot time.",
				partition: "Whole observed counter intervals; crossing a minute closes at the actual receipt, with no interpolation. Final partial retained. Reset intervals may lose pre-reset ticks.",
				windows: rows,
			};
			rows = []; overflow = 0; firstOverflow = null;
			row = diagnosticRow(current, at);
			previousAt = validObservationClock(at) ? at : null;
			return report;
		},
	};
}
function validObservationClock(at) {
	return Number.isSafeInteger(at) && at >= 0 && at <= 8_640_000_000_000_000;
}
function sameCoarseGeometry(first, next) {
	const bounds = next.tick_duration_bucket_bounds_micros;
	const previous = first.tick_duration_bucket_bounds_micros;
	return Array.isArray(bounds) && Array.isArray(previous) && bounds.length > 0
		&& Array.isArray(next.tick_duration_bucket_counts)
		&& bounds.length === next.tick_duration_bucket_counts.length
		&& bounds.length === previous.length
		&& bounds.every((bound, index) => Number.isSafeInteger(bound) && bound > 0
			&& (index === 0 || bound > bounds[index - 1]) && bound === previous[index]);
}
function diagnosticRow(snapshot, at) {
	return {
		startedAtEpochMillis: validObservationClock(at) ? at : null,
		endedAtEpochMillis: validObservationClock(at) ? at : null,
		sampleIntervals: 0, counterResets: 0, samplingErrors: 0,
		invalidClockIntervals: validObservationClock(at) ? 0 : 1,
		coarseGeometryDriftIntervals: sameCoarseGeometry(snapshot, snapshot) ? 0 : 1,
		maximumObservationGapMillis: 0, gapsAboveFiveSeconds: 0,
		output: emptyWindow(snapshot),
	};
}
function finishDiagnosticRow(row, finalPartial) {
	const reasons = [];
	if (row.invalidClockIntervals) reasons.push("missing, invalid or reversed client observation clock");
	if (row.coarseGeometryDriftIntervals) reasons.push("coarse histogram geometry changed or is invalid during this diagnostic window");
	if (row.counterResets) reasons.push("counter reset: ticks before reset after the previous observation are unavailable");
	if (row.samplingErrors) reasons.push("counter snapshot request failed");
	if (row.gapsAboveFiveSeconds) reasons.push("observation gap exceeded five seconds; temporal attribution is coarse");
	const fine = fineState(row.output);
	if (fine.status !== "complete") reasons.push(...fine.reasons);
	else if (fine.histogram.sample_count === 0) reasons.push("no output ticks observed in this diagnostic window");
	const { output, ...metadata } = row;
	return {
		...metadata, finalPartial,
		observationCount: metadata.sampleIntervals + 1,
		durationMillis: metadata.startedAtEpochMillis === null || metadata.endedAtEpochMillis === null
			|| metadata.endedAtEpochMillis < metadata.startedAtEpochMillis
			? null : metadata.endedAtEpochMillis - metadata.startedAtEpochMillis,
		coverage: { status: reasons.length ? "unavailable" : "complete", reasons: [...new Set(reasons)] },
		framesSent: output.frames_sent, packetsSent: output.packets_sent,
		deadlineMisses: output.deadline_misses, sendErrors: output.send_errors,
		fineHistogramCoverage: output.fine_histogram_coverage,
		...(output.tick_duration_fine_histogram ? { fineHistogram: output.tick_duration_fine_histogram } : {}),
		coarseBoundsMicros: output.tick_duration_bucket_bounds_micros,
		coarseCounts: output.tick_duration_bucket_counts,
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
        ...(snapshot.tick_duration_fine_histogram ? { tick_duration_fine_histogram: {
            ...snapshot.tick_duration_fine_histogram,
            counts: Array.isArray(snapshot.tick_duration_fine_histogram.counts)
                ? snapshot.tick_duration_fine_histogram.counts.map(() => 0)
                : snapshot.tick_duration_fine_histogram.counts,
            sample_count: 0,
        }} : {}),
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
        ...addFineWindows(total, next),
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


const FINE_BOUNDS = Object.freeze([
    ...Array.from({length:128},(_,index)=>(index+1)*250),
    48_000,64_000,100_000,1_000_000,Number.MAX_SAFE_INTEGER,
]);
function safeSum(counts) {
    if (!Array.isArray(counts)) return null;
    for (const count of counts) if (!Number.isSafeInteger(count) || count<0) return null;
    const sum = counts.reduce((total,count)=>total+count,0);
    return Number.isSafeInteger(sum) ? sum : null;
}
function fineState(snapshot) {
    if (snapshot.fine_histogram_coverage?.status === "invalid") {
        return {status:"invalid",reasons:Array.isArray(snapshot.fine_histogram_coverage.reasons) ? snapshot.fine_histogram_coverage.reasons.filter(reason=>typeof reason === "string") : ["recorded fine coverage invalid"]};
    }
    const coarse = safeSum(snapshot.tick_duration_bucket_counts);
    if (coarse === null) return {status:"invalid",reasons:["invalid coarse sample count"]};
    const fine = snapshot.tick_duration_fine_histogram;
    if (fine == null) return {status:coarse === 0 ? "empty" : "unavailable",reasons:["fine histogram absent"]};
    if (fine.schema_version !== 1) return {status:"invalid",reasons:["unsupported fine histogram version"]};
    if (!Array.isArray(fine.bounds_micros) || fine.bounds_micros.length !== FINE_BOUNDS.length ||
        FINE_BOUNDS.some((bound,index)=>fine.bounds_micros[index] !== bound)) {
        return {status:"invalid",reasons:["fine histogram geometry drift"]};
    }
    const sum = safeSum(fine.counts);
    if (fine.counts?.length !== FINE_BOUNDS.length || sum === null ||
        !Number.isSafeInteger(fine.sample_count) || fine.sample_count !== sum || sum !== coarse) {
        return {status:"invalid",reasons:["incomplete or malformed fine sample coverage"]};
    }
    return {status:"complete",histogram:fine,reasons:[]};
}
function fineResult(state) {
    return {
        fine_histogram_coverage:{status:state.status === "empty" ? "unavailable" : state.status,reasons:state.reasons},
        ...(state.status === "complete" ? {tick_duration_fine_histogram:{
            schema_version:1,bounds_micros:[...FINE_BOUNDS],counts:[...state.histogram.counts],sample_count:state.histogram.sample_count,
        }} : {}),
    };
}
function rejectedFine(...states) {
    return fineResult({status:"invalid",reasons:[...new Set(states.flatMap(state=>state.reasons))]});
}
function fineWindow(before,after) {
    const first = fineState(before);
    const last = fineState(after);
    if (first.status === "invalid" || last.status === "invalid") return rejectedFine(first,last);
    if ([first,last].every(state=>state.status === "empty" || state.status === "unavailable")) {
        return fineResult({status:"unavailable",reasons:["fine histogram absent throughout window"]});
    }
    if (last.status !== "complete" || !["complete","empty"].includes(first.status)) {
        return rejectedFine({reasons:["fine histogram missing at nonempty window boundary"]});
    }
    const counts = last.histogram.counts.map((count,index)=>count-(first.histogram?.counts[index] ?? 0));
    const sum = safeSum(counts);
    if (sum === null || sum !== safeSum(after.tick_duration_bucket_counts)-safeSum(before.tick_duration_bucket_counts)) {
        return rejectedFine({reasons:["fine counters changed without matching coarse window"]});
    }
    return fineResult({status:"complete",histogram:{counts,sample_count:sum},reasons:[]});
}
function addFineWindows(total,next) {
    const first = fineState(total);
    const last = fineState(next);
    if (first.status === "invalid" || last.status === "invalid") return rejectedFine(first,last);
    // An empty missing boundary is normal: the backend deliberately omits zero totals.
    if (first.status === "empty") return fineResult(last);
    if (last.status === "empty") return fineResult(first);
    if (first.status === "unavailable" && last.status === "unavailable") return fineResult(first);
    if (first.status !== "complete" || last.status !== "complete") {
        return rejectedFine({reasons:["fine histogram missing during accumulated window"]});
    }
    const counts = first.histogram.counts.map((count,index)=>count+last.histogram.counts[index]);
    const sum = safeSum(counts);
    if (sum === null) return rejectedFine({reasons:["fine accumulated counters exceed safe integer range"]});
    return fineResult({status:"complete",histogram:{counts,sample_count:sum},reasons:[]});
}
function finePercentileInterval(state,percentile) {
    if (state.status !== "complete" || state.histogram.sample_count === 0) return null;
    const rank = Math.ceil(percentile/100*state.histogram.sample_count);
    let cumulative = 0;
    for (let index=0;index<FINE_BOUNDS.length;index++) {
        cumulative += state.histogram.counts[index];
        if (cumulative >= rank) return [index === 0 ? 0 : FINE_BOUNDS[index-1]+1,index === FINE_BOUNDS.length-1 ? null : FINE_BOUNDS[index]];
    }
    return null;
}
export function compareOutputWindows(noStage,stage) {
    const noStageP99TickMicros = histogramPercentileMicros(noStage,99);
    const stageP99TickMicros = histogramPercentileMicros(stage,99);
    const allowedP99RegressionMicros = noStageP99TickMicros === null ? null : Math.max(1000,noStageP99TickMicros*0.05);
    const p99RegressionMicros = noStageP99TickMicros === null || stageP99TickMicros === null ? null : stageP99TickMicros-noStageP99TickMicros;
    const coarsePassed = p99RegressionMicros !== null && allowedP99RegressionMicros !== null && p99RegressionMicros <= allowedP99RegressionMicros;
    const control = fineState(noStage);
    const candidate = fineState(stage);
    const legacy = [control,candidate].every(state=>state.status === "unavailable" || state.status === "empty");
    const controlInterval = finePercentileInterval(control,99);
    const stageInterval = finePercentileInterval(candidate,99);
    let fineP99Comparison = {status:legacy ? "unavailable" : "invalid",decision:"unavailable",
        reasons:[...new Set([...control.reasons,...candidate.reasons])],
        noStageIntervalMicros:controlInterval,stageIntervalMicros:stageInterval,
        regressionIntervalMicros:null,allowedRegressionIntervalMicros:null,
        nominalFineBucketMicros:250,intervalConvention:"inclusive integer microseconds; null upper means overflow"};
    if (controlInterval && stageInterval && (controlInterval[1] === null || stageInterval[1] === null)) {
        fineP99Comparison = {...fineP99Comparison,status:"complete",decision:"indeterminate",reasons:["fine p99 reaches unbounded overflow bucket"]};
    } else if (controlInterval && stageInterval) {
        const regression = [stageInterval[0]-controlInterval[1],stageInterval[1]-controlInterval[0]];
        const threshold = controlInterval.map(bound=>Math.max(1000,bound*0.05));
        const decision = regression[1] <= threshold[0] ? "pass" : regression[0] > threshold[1] ? "fail" : "indeterminate";
        fineP99Comparison = {...fineP99Comparison,status:"complete",decision,reasons:[],
            regressionIntervalMicros:regression,allowedRegressionIntervalMicros:threshold};
    } else if (!legacy && fineP99Comparison.reasons.length === 0) {
        fineP99Comparison.reasons = ["fine comparison requires nonempty complete coverage in both windows"];
    }
    return {noStageP99TickMicros,stageP99TickMicros,allowedP99RegressionMicros,p99RegressionMicros,
        coarseBoundedWindowGatePassed:coarsePassed,fineP99Comparison,
        p99ComparisonSource:legacy ? "coarse_histogram_fallback" : fineP99Comparison.status === "invalid" ? "fine_histogram_rejected" : "fine_histogram_v1",
        boundedWindowGatePassed:legacy ? coarsePassed : fineP99Comparison.decision === "pass"};
}
