import { useEffect, useState } from "react";
import type { TimecodeDefinition } from "../../api/types/timecode";
import type { TimecodeEditorSelection } from "./editorModel";
import type { TimelineItem } from "./timecodeEditorShared";

/** Section folding changes only editor visibility and preserves the shared selection model. */
export function useTimelineSections({
	definition,
	activeLaneId,
	items,
	setSelection,
	setSelectedLaneId,
	setSelectedCueId,
	prepareInspectorClose,
}: {
	definition: TimecodeDefinition;
	activeLaneId: string | null;
	items: readonly TimelineItem[];
	setSelection(selection: TimecodeEditorSelection | null): void;
	setSelectedLaneId(id: string | null): void;
	setSelectedCueId(id: string | null): void;
	prepareInspectorClose(): void;
}) {
	const [expandedSpeedLaneId, setExpandedSpeedLaneId] = useState<string | null>(null);
	const [collapsedSections, setCollapsedSections] = useState<
		Record<string, boolean>
	>({});
	useEffect(() => {
		setCollapsedSections({});
		setExpandedSpeedLaneId(null);
	}, [definition.id]);
	useEffect(() => {
		const kind = definition.lanes.find((lane) => lane.id === activeLaneId)
			?.content.kind;
		if (kind)
			setCollapsedSections((current) =>
				current[kind] ? { ...current, [kind]: false } : current,
			);
	}, [activeLaneId, definition.lanes]);
	const onCloseCue = () => {
		setExpandedSpeedLaneId(null);
		prepareInspectorClose();
		setSelectedCueId(null);
		setSelectedLaneId(null);
		setSelection(null);
	};
	const onToggleSection = (kind: string) => {
		const closing = !collapsedSections[kind];
		if (
			closing &&
			definition.lanes.find((lane) => lane.id === activeLaneId)?.content
				.kind === kind
		)
			onCloseCue();
		setCollapsedSections((current) => ({ ...current, [kind]: closing }));
	};
	const onSelectLane = (laneId: string) => {
		const kind = definition.lanes.find((lane) => lane.id === laneId)?.content
			.kind;
		if (kind)
			setCollapsedSections((current) => ({ ...current, [kind]: false }));
		setExpandedSpeedLaneId(kind === "speed_group" ? laneId : null);
		setSelectedLaneId(laneId);
		setSelection(
			items.find((item) => item.laneId === laneId)?.selection ?? null,
		);
	};
	return { collapsedSections, onToggleSection, onSelectLane, onCloseCue, expandedSpeedLaneId };
}

/** The last selected lane remains addressable when its selected item is cleared. */
export function useTimelineSelectedLane(
	selection: TimecodeEditorSelection | null,
): [string | null, (id: string | null) => void] {
	const [laneId, setLaneId] = useState<string | null>(null);
	useEffect(() => {
		if (selection && "laneId" in selection) setLaneId(selection.laneId);
	}, [selection]);
	return [laneId, setLaneId];
}
