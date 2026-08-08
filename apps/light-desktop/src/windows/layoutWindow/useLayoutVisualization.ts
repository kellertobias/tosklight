import { useEffect, useMemo, useState } from "react";
import type { VisualizationSnapshot } from "../../api/types";
import { usePatchedFixturesView } from "../../features/patch/PatchState";
import { useVisualizationRuntimeRead } from "../../features/visualizationRuntime/VisualizationRuntimeView";
import { fixturePresentation } from "./fixturePresentation";

/**
 * Eventually-consistent, one-request-at-a-time Layout projection.
 *
 * Only the configured Group's fixture IDs cross the visualization route. A slow response delays
 * the next read instead of building a queue, so Layout cannot contend with programmer/output work.
 */
export function useLayoutVisualization(
	active: boolean,
	fixtureIds: readonly string[],
	intervalMillis = 100,
) {
	const fixtures = usePatchedFixturesView(active);
	const fixtureScope = fixtureIds.join(",");
	const requested = useMemo(() => new Set(fixtureIds), [fixtureScope]);
	const scopedFixtures = useMemo(
		() =>
			fixtures.filter(
				(fixture) =>
					requested.has(fixture.fixture_id) ||
					fixture.logical_heads.some((head) => requested.has(head.fixture_id)),
			),
		[fixtures, requested],
	);
	const read = useVisualizationRuntimeRead("normal", { fixtureIds });
	const [snapshot, setSnapshot] = useState<VisualizationSnapshot | null>(null);
	useEffect(() => {
		if (!active || !fixtureScope) {
			setSnapshot(null);
			return;
		}
		let cancelled = false;
		let timer: number | null = null;
		const poll = async () => {
			try {
				const next = await read();
				if (!cancelled) setSnapshot(next);
			} finally {
				if (!cancelled) timer = window.setTimeout(poll, intervalMillis);
			}
		};
		void poll();
		return () => {
			cancelled = true;
			if (timer !== null) window.clearTimeout(timer);
		};
	}, [active, fixtureScope, intervalMillis, read]);
	const presentations = useMemo(
		() =>
			scopedFixtures.map((fixture, index) =>
				fixturePresentation(fixture, index, snapshot, false),
			),
		[scopedFixtures, snapshot],
	);
	return { fixtures: scopedFixtures, presentations, snapshot };
}
