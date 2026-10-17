import { useEffect, useRef, useState } from "react";
import type { VisualizationSnapshot } from "../../api/types";
import { useProgrammerPreloadValuesView } from "../../features/programmerPreloadValues/ProgrammerPreloadValuesView";
import type { ProgrammerFixtureValue } from "../../features/programmerValues/contracts";
import { useProgrammerValuesView } from "../../features/programmerValues/ProgrammerValuesView";
import { useVisualizationRuntimeRead } from "../../features/visualizationRuntime/VisualizationRuntimeView";

interface SelectedMediaValues {
	fixtureId: string | undefined;
	sourceKey: string;
	output: VisualizationSnapshot | null;
}

/** Pending capture overrides are shown without writing them into the live Programmer. */
export function useMediaProgrammerValues(
	active: boolean,
	route: "normal" | "preload" | null,
	selected?: SelectedMediaValues,
) {
	const resolved = useSelectedMediaValues(active, selected);
	const normal = useProgrammerValuesView(active);
	const pending = useProgrammerPreloadValuesView(active && route === "preload");
	const values = mediaProgrammerValues(
		normal?.fixtureValues,
		route === "preload" ? pending?.fixtureValues : undefined,
		resolved,
	);
	if (!active) return undefined;
	return selected
		? values?.filter((value) => value.fixtureId === selected.fixtureId)
		: values;
}

export function mediaProgrammerValues(
	normal?: readonly ProgrammerFixtureValue[],
	pending?: readonly ProgrammerFixtureValue[],
	resolved?: readonly ProgrammerFixtureValue[],
) {
	if (!resolved && !pending?.length) return normal;
	const values = new Map(
		(resolved ?? []).map((value) => [
			`${value.fixtureId}:${value.attribute}`,
			value,
		]),
	);
	for (const value of [...(normal ?? []), ...(pending ?? [])])
		values.set(`${value.fixtureId}:${value.attribute}`, value);
	return [...values.values()];
}

/** Selected reads follow the existing pushed Normal lane, never an independent polling timer. */
function useSelectedMediaValues(
	active: boolean,
	selected?: SelectedMediaValues,
) {
	const fixtureId = selected?.fixtureId;
	const sourceKey = selected?.sourceKey ?? "";
	const output = selected?.output;
	const read = useVisualizationRuntimeRead("normal", {
		fixtureIds: fixtureId ? [fixtureId] : [],
	});
	const available = Boolean(active && fixtureId && output && !output.preload);
	const [captured, setCaptured] = useState<{
		read: typeof read;
		key: string;
		values: ProgrammerFixtureValue[];
	} | null>(null);
	const refresh = useRef<(() => void) | null>(null);
	const key = JSON.stringify([sourceKey, fixtureId, output?.scope?.show_id]);
	useEffect(() => {
		if (!available || !fixtureId) {
			setCaptured(null);
			return;
		}
		let cancelled = false;
		let inFlight = false;
		const load = async () => {
			if (inFlight) return;
			inFlight = true;
			try {
				const next = await read();
				if (cancelled) return;
				if (
					next.preload ||
					(output?.scope?.show_id &&
						next.scope?.show_id !== output.scope.show_id)
				) {
					setCaptured(null);
					return;
				}
				setCaptured({
					read,
					key,
					values: next.values
						.filter((value) => value.fixture_id === fixtureId)
						.map((value) => ({
							fixtureId,
							attribute: value.attribute,
							value: value.value,
							programmerOrder: 0,
							fade: false,
							fadeMillis: null,
							delayMillis: null,
						})),
				});
			} catch {
				if (!cancelled) setCaptured(null);
			} finally {
				inFlight = false;
			}
		};
		refresh.current = () => void load();
		refresh.current();
		return () => {
			cancelled = true;
			refresh.current = null;
		};
	}, [available, fixtureId, key, read]);
	useEffect(() => {
		refresh.current?.();
	}, [output]);
	return available && captured?.read === read && captured.key === key
		? captured.values
		: undefined;
}
