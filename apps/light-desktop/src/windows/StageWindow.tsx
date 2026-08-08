import type { HighlightState } from "../api/types";
import { GroupStrip } from "../components/shared/GroupStrip";
import { useHighlightSnapshot } from "../features/highlight/HighlightState";
import { StageHeader } from "./stageWindow/StageHeader";
import { StageRendererView } from "./stageWindow/StageRendererView";
import type { StageWindowProps } from "./stageWindow/types";
import { useStageOptions } from "./stageWindow/useStageOptions";
import { useStageSelection } from "./stageWindow/useStageSelection";

/**
 * The Stage: a spatial selection surface with the renderer's picture in it.
 *
 * Nothing here is told what the rig is doing. The renderer reads the desk's own output universes
 * and draws from those, so the interface is no longer sent live values several dozen times a
 * second for a picture it does not draw. What this owns is the operator's side of the pane — which
 * view, which fixtures are selected, whether the Group strip is up.
 */
export function StageWindow(props: StageWindowProps) {
	const active = props.active ?? true;
	const interactionActive = active && !props.viewOnly;
	const options = useStageOptions(props);
	const selection = useStageSelection(interactionActive);
	const highlight = useHighlightSnapshot();
	const highlightFixtures = options.followPreload
		? []
		: stageHighlightFixtureIds(highlight);
	return (
		<div
			className={`stage-window ${props.compact ? "compact" : ""}`}
			data-visualization-lane={options.followPreload ? "preload" : "live"}
		>
			{!props.compact && (
				<StageHeader
					options={options}
					selectedCount={selection.fixtureIds.length}
				/>
			)}
			{/*
			 * Every Stage is the renderer's picture — the plan, the outline view and the full one
			 * alike. A 2D Stage is one of the renderer's own orthographic views of the rig rather
			 * than a second arrangement of it, which is why the operator chooses a side to look
			 * from instead of a saved layout to look at.
			 */}
			<StageRendererView
				options={options}
				selection={selection}
				highlightFixtures={highlightFixtures}
				active={active}
				interactive={!props.viewOnly}
			/>
			{options.groupsVisible && <GroupStrip active={active} />}
		</div>
	);
}

/** Immediate Stage-only projection of the authoritative Highlight selection. */
export function stageHighlightFixtureIds(
	highlight: HighlightState | null,
): string[] {
	if (!highlight?.active || !highlight.output_enabled) return [];
	if (highlight.mode === "step")
		return highlight.active_fixture
			? [highlight.active_fixture.fixture_id]
			: [];
	return highlight.remembered.map((fixture) => fixture.fixture_id);
}
