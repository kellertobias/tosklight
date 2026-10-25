import type { InstalledFixtureAppearance } from "../../../api/types";
import { manualOpticsFromWire } from "../../../api/manualOptics";
import type { PatchInstalledFixtureAppearance } from "../../../features/patch/contracts";

export interface ManualOpticsDraft {
	manualZoomDegrees: string;
	manualShaperBlades: [string, string, string, string];
	manualOpticsEdited: boolean;
}

export function manualOpticsDraft(
	appearance: InstalledFixtureAppearance,
): ManualOpticsDraft {
	return {
		manualZoomDegrees:
			appearance.manual_optics?.zoom_degrees == null
				? ""
				: String(appearance.manual_optics.zoom_degrees),
		manualShaperBlades: (
			appearance.manual_optics?.shaper_blades ?? [0, 0, 0, 0]
		).map((value) => String(value * 100)) as ManualOpticsDraft["manualShaperBlades"],
		manualOpticsEdited: false,
	};
}

export function normalizeManualOptics(
	draft: ManualOpticsDraft,
	baseline: InstalledFixtureAppearance,
): { fields: Pick<PatchInstalledFixtureAppearance, "manualOptics"> } | { error: string } {
	if (!draft.manualOpticsEdited) return { fields: manualOpticsFromWire(baseline) };
	const rawZoom = draft.manualZoomDegrees.trim();
	const zoomDegrees = rawZoom === "" ? null : Number(rawZoom);
	if (
		zoomDegrees !== null &&
		(!Number.isFinite(zoomDegrees) || zoomDegrees < 1 || zoomDegrees > 170)
	)
		return {
			error:
				"Manual zoom full opening must be from 1° to 170°, or empty to inherit the profile.",
		};
	const blades = draft.manualShaperBlades.map((value) =>
		value.trim() === "" ? NaN : Number(value),
	);
	if (blades.some((value) => !Number.isFinite(value) || value < 0 || value > 100))
		return { error: "Each manual blade closure must be from 0% to 100%." };
	return {
		fields: {
			manualOptics: {
				zoomDegrees,
				shaperBlades: blades.map((value) => value / 100) as [
					number, number, number, number,
				],
			},
		},
	};
}
