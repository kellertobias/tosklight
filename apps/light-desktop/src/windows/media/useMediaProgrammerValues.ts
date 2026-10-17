import {
	useCallback,
	useEffect,
	useMemo,
	useRef,
	useState,
	useSyncExternalStore,
} from "react";
import type { PatchedFixture, VisualizationSnapshot } from "../../api/types";
import { usePatchStoreOrNull } from "../../features/patch/PatchContext";
import {
	usePatchStatus,
	useSelectedPatchedFixtures,
} from "../../features/patch/PatchState";
import { useProgrammerPreloadValuesView } from "../../features/programmerPreloadValues/ProgrammerPreloadValuesView";
import type { ProgrammerFixtureValue } from "../../features/programmerValues/contracts";
import { useProgrammerValuesView } from "../../features/programmerValues/ProgrammerValuesView";
import { useVisualizationRuntimeRead } from "../../features/visualizationRuntime/VisualizationRuntimeView";

interface SelectedMediaValues {
	fixtureId: string | undefined;
	parentFixtureId?: string;
	expectedShowId?: string;
	sourceKey: string;
	output: VisualizationSnapshot | null;
}

/** Pending capture overrides are shown without writing them into the live Programmer. */
export function useMediaProgrammerValues(
	active: boolean,
	route: "normal" | "preload" | null,
	selected?: SelectedMediaValues,
) {
	const defaults = useSelectedMediaDefaults(active, selected);
	const resolved = useSelectedMediaValues(
		active,
		selected
			? {
					...selected,
					sourceKey: `${selected.sourceKey}:${defaults?.key ?? "no-profile-defaults"}`,
					expectedShowId: defaults?.showId,
				}
			: undefined,
	);
	const normal = useProgrammerValuesView(active);
	const pending = useProgrammerPreloadValuesView(active && route === "preload");
	const values = mediaProgrammerValues(
		normal?.fixtureValues,
		route === "preload" ? pending?.fixtureValues : undefined,
		resolved,
		defaults?.values,
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
	defaults?: readonly ProgrammerFixtureValue[],
) {
	if (!resolved && !pending?.length) return normal;
	const values = new Map(
		[
			...(resolved !== undefined ? (defaults ?? []) : []),
			...(resolved ?? []),
		].map((value) => [`${value.fixtureId}:${value.attribute}`, value]),
	);
	for (const value of [...(normal ?? []), ...(pending ?? [])])
		values.set(`${value.fixtureId}:${value.attribute}`, value);
	return [...values.values()];
}

const noSubscription = () => () => {};

/** Exact canonical homes from current Patch authority, never a guessed Media personality. */
function useSelectedMediaDefaults(
	active: boolean,
	selected?: SelectedMediaValues,
) {
	const parentId = selected?.parentFixtureId;
	const fixtures = useSelectedPatchedFixtures(
		parentId ? [parentId] : [],
		active && Boolean(parentId),
	);
	const status = usePatchStatus(active && Boolean(parentId));
	const store = usePatchStoreOrNull();
	const getAuthority = useCallback(() => {
		const snapshot = store?.getSnapshot();
		return JSON.stringify([
			snapshot?.showId ?? null,
			Boolean(parentId && snapshot?.pendingFixtureIds.has(parentId)),
		]);
	}, [store, parentId]);
	const authority = useSyncExternalStore(
		active && parentId && store ? store.subscribe : noSubscription,
		getAuthority,
		getAuthority,
	);
	const [showId, pending] = JSON.parse(authority) as [string | null, boolean];
	const fixture = fixtures.find(
		(candidate) => candidate.fixture_id === parentId,
	);
	const outputShowId = selected?.output?.scope?.show_id;
	return useMemo(() => {
		if (
			!active ||
			status.status !== "ready" ||
			status.error ||
			pending ||
			!showId ||
			(outputShowId && outputShowId !== showId)
		)
			return undefined;
		const defaults = mediaHeadDefaults(fixture, selected?.fixtureId);
		return defaults
			? { ...defaults, showId, key: JSON.stringify([showId, defaults.key]) }
			: undefined;
	}, [
		active,
		status.status,
		status.error,
		pending,
		showId,
		outputShowId,
		fixture,
		selected?.fixtureId,
	]);
}

function mediaHeadDefaults(
	fixture: PatchedFixture | undefined,
	fixtureId: string | undefined,
) {
	if (!fixture || !fixtureId) return undefined;
	const definition = fixture.definition;
	if (
		!definition.profile_id ||
		!definition.mode_id ||
		!Number.isSafeInteger(definition.revision) ||
		definition.revision < 0
	)
		return undefined;
	const logical = fixture.logical_heads.find(
		(head) => head.fixture_id === fixtureId,
	);
	const master = fixture.fixture_id === fixtureId;
	if (!master && !logical) return undefined;
	const mode = definition.profile_snapshot?.modes.find(
		(candidate) => candidate.id === definition.mode_id,
	);
	if (
		definition.profile_snapshot &&
		(definition.profile_snapshot.id !== definition.profile_id ||
			definition.profile_snapshot.revision !== definition.revision ||
			!mode)
	)
		return undefined;
	if (
		logical?.profile_head_id &&
		mode?.heads[logical.head_index]?.id !== logical.profile_head_id &&
		mode
	)
		return undefined;
	const heads = definition.heads.filter((head) =>
		master ? head.shared : !head.shared && head.index === logical?.head_index,
	);
	if (!heads.length) return undefined;
	const values = new Map<string, ProgrammerFixtureValue>();
	for (const parameter of heads.flatMap((head) => head.parameters)) {
		const value = parameter.default;
		if (!Number.isFinite(value) || value < 0 || value > 1) continue;
		const previous = values.get(parameter.attribute)?.value;
		if (previous?.kind === "normalized" && previous.value !== value)
			return undefined;
		values.set(parameter.attribute, {
			fixtureId,
			attribute: parameter.attribute,
			value: { kind: "normalized", value },
			programmerOrder: 0,
			fade: false,
			fadeMillis: null,
			delayMillis: null,
		});
	}
	return {
		key: JSON.stringify([
			fixture.fixture_id,
			definition.profile_id,
			definition.revision,
			definition.mode_id,
			fixtureId,
			logical?.profile_head_id,
			logical?.head_index,
			heads.map((head) => head.index),
		]),
		values: [...values.values()],
	};
}

/** Selected reads follow the existing pushed Normal lane, never an independent polling timer. */
function useSelectedMediaValues(
	active: boolean,
	selected?: SelectedMediaValues,
) {
	const fixtureId = selected?.fixtureId;
	const sourceKey = selected?.sourceKey ?? "";
	const output = selected?.output;
	const showId = output?.scope?.show_id ?? selected?.expectedShowId;
	const read = useVisualizationRuntimeRead("normal", {
		fixtureIds: fixtureId ? [fixtureId] : [],
	});
	const available = Boolean(active && fixtureId && output && !output.preload);
	const [captured, setCaptured] = useState<{
		read: typeof read;
		key: object;
		values: ProgrammerFixtureValue[];
	} | null>(null);
	const refresh = useRef<(() => void) | null>(null);
	const key = useMemo(() => ({}), [sourceKey, fixtureId, showId]);
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
				if (next.preload || (showId && next.scope?.show_id !== showId)) {
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
