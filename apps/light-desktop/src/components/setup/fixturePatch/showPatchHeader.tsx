import type { TitleActionGroup } from "@tosklight/ui";
import { WindowHeader } from "@tosklight/ui/window-kit";
import { useState } from "react";
import {
	ShowPatchSettings,
	type ShowPatchSettingsTab,
} from "./ShowPatchSettings";

export type ShowPatchView = "fixtures" | "media" | "tracking";

const VIEW_LABELS: Record<ShowPatchView, string> = {
	fixtures: "Fixtures",
	media: "Media Servers",
	tracking: "Tracking",
};

/**
 * The Fixtures / Media Servers / Tracking switch, built the same way on every Show Patch view so
 * it keeps its place and width when the operator moves between them.
 */
export function showPatchViewGroup(
	active: ShowPatchView,
	views: readonly ShowPatchView[],
	onChange: (view: ShowPatchView) => void,
): TitleActionGroup {
	return {
		id: "patch-kind",
		kind: "tabs",
		activeId: active,
		onActiveChange: (id) => {
			if (id !== active) onChange(id as ShowPatchView);
		},
		actions: views.map((view) => ({ id: view, label: VIEW_LABELS[view] })),
	};
}

/** Which Settings page the ⚙ opens first on a view. */
export function settingsTabFor(view: ShowPatchView): ShowPatchSettingsTab {
	return view === "tracking" ? "tracking" : "columns";
}

/** Header for the Media Servers and Tracking views; Fixtures adds its own actions to the same shape. */
export function ShowPatchViewHeader({
	view,
	compact,
	onView,
}: {
	view: Exclude<ShowPatchView, "fixtures">;
	compact: boolean;
	onView: (view: ShowPatchView) => void;
}) {
	const [anchor, setAnchor] = useState<DOMRect | null>(null);
	return (
		<>
			<WindowHeader
				title="Show Patch"
				settings={!compact}
				onSettings={(button) => setAnchor(button.getBoundingClientRect())}
				info={{ primary: VIEW_LABELS[view] }}
				groups={[
					showPatchViewGroup(view, ["fixtures", "media", "tracking"], onView),
				]}
			/>
			{anchor ? (
				<ShowPatchSettings
					anchor={anchor}
					initialTab={settingsTabFor(view)}
					onClose={() => setAnchor(null)}
				/>
			) : null}
		</>
	);
}
