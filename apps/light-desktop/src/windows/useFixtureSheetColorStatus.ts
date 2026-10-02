import { useMemo } from "react";
import { fixtureColorNotices } from "../features/colorReport/acceptedColorReport";
import {
	colorDetailsRequests,
	useAcceptedColorReport,
} from "../features/colorReport/useAcceptedColorReport";
import { useSemanticFamilyEncoders } from "../features/familyEncoders/FamilyEncodersProvider";
import { useApp } from "../state/AppContext";
import type { FixtureSheetColumn, FixtureSheetCompactMode } from "../types";
import {
	type FixtureSheetColorStatus,
	fixtureSheetColumns,
} from "./fixtureSheetColumns";
import type { FixtureStepPresenter } from "./fixtureSheetStep";


const NO_FIXTURES: readonly string[] = [];

/**
 * The Fixture Sheet's quiet Color status (TL-550): one batched accepted-frame colour report for
 * the rows on screen, re-read with the Programmer projection (throttled), never per row. Only
 * under the semantic programming contract; at the legacy contract nothing is read or shown.
 * Activating a row's triangle opens the Color Special Dialog on its per-fixture details.
 */
export function useFixtureSheetColorStatus(
	active: boolean,
	visibleFixtureIds: readonly string[],
	refreshKey: unknown,
): FixtureSheetColorStatus | undefined {
	const { dispatch } = useApp();
	const semantic = useSemanticFamilyEncoders(NO_FIXTURES, active);
	const report = useAcceptedColorReport(visibleFixtureIds, {
		enabled: active && semantic,
		refreshKey,
	});
	const notices = useMemo(() => fixtureColorNotices(report), [report]);
	return useMemo(
		() =>
			notices.size
				? {
						notice: (row) => notices.get(row.fixtureId) ?? null,
						openDetails: (row) => {
							colorDetailsRequests.request(row.fixtureId);
							dispatch({ type: "OPEN_SPECIAL_DIALOG", family: "Color" });
						},
					}
				: undefined,
		[dispatch, notices],
	);
}

/** The sheet's visible columns, with the quiet Color status when the Color column is shown. */
export function useFixtureSheetColumns(
	active: boolean,
	visibleFixtureIds: readonly string[],
	options: {
		refreshKey: unknown;
		showType: boolean;
		presentStep: FixtureStepPresenter;
		compactMode: FixtureSheetCompactMode;
		visibleColumnIds: readonly FixtureSheetColumn[];
	},
) {
	const { refreshKey, showType, presentStep, compactMode, visibleColumnIds } =
		options;
	const colorStatus = useFixtureSheetColorStatus(
		active && visibleColumnIds.includes("color"),
		visibleFixtureIds,
		refreshKey,
	);
	return useMemo(
		() =>
			fixtureSheetColumns(showType, presentStep, compactMode, colorStatus).filter(
				(column) => visibleColumnIds.includes(column.id as FixtureSheetColumn),
			),
		[colorStatus, compactMode, presentStep, showType, visibleColumnIds],
	);
}
