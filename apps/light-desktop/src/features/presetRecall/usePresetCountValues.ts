import { useEffect, useMemo } from "react";
import type { VisualizationSnapshot } from "../../api/types";
import { useProgrammerValuesView } from "../programmerValues/ProgrammerValuesView";
import { useProgrammerCaptureModeView } from "../programmerCaptureMode/ProgrammerCaptureModeView";
import {
	resolvedValueIndex,
	type ResolvedValueIndex,
} from "../presetRecording/presetFixtureCounts";
import { usePresetCountFeedback } from "./PresetRecallProvider";
import {
	recalledCountValues,
	withRecalledCountValues,
} from "./countProjection";

const empty: ResolvedValueIndex = new Map();

/** Counts react on the accepted WebSocket turn; output remains the reconciled source of truth. */
export function usePresetCountValues(
	visualization: VisualizationSnapshot | null,
	groups: ReadonlyMap<string, readonly string[]>,
	enabled: boolean,
) {
	const { store, feedback } = usePresetCountFeedback(enabled);
	const programmer = useProgrammerValuesView(enabled);
	const capture = useProgrammerCaptureModeView(enabled);
	const live = Boolean(capture && !capture.blind && !capture.preview);
	const current = live && programmer?.revision === feedback?.after.revision;
	const changed = useMemo(
		() => (feedback && current ? recalledCountValues(feedback, groups) : empty),
		[feedback, current, groups],
	);
	useEffect(() => {
		if (feedback && !current) store?.clear();
		else store?.observe(visualization, changed);
	}, [changed, current, feedback, store, visualization]);
	const resolved = useMemo(
		() => resolvedValueIndex(visualization),
		[visualization],
	);
	return useMemo(
		() => withRecalledCountValues(resolved, changed),
		[resolved, changed],
	);
}
