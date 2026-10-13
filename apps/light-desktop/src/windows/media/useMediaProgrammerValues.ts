import { useProgrammerPreloadValuesView } from "../../features/programmerPreloadValues/ProgrammerPreloadValuesView";
import type { ProgrammerFixtureValue } from "../../features/programmerValues/contracts";
import { useProgrammerValuesView } from "../../features/programmerValues/ProgrammerValuesView";

/** Pending capture overrides are shown without writing them into the live Programmer. */
export function useMediaProgrammerValues(
	active: boolean,
	route: "normal" | "preload" | null,
) {
	const normal = useProgrammerValuesView(active);
	const pending = useProgrammerPreloadValuesView(active && route === "preload");
	return mediaProgrammerValues(
		normal?.fixtureValues,
		route === "preload" ? pending?.fixtureValues : undefined,
	);
}

export function mediaProgrammerValues(
	normal?: readonly ProgrammerFixtureValue[],
	pending?: readonly ProgrammerFixtureValue[],
) {
	if (!pending?.length) return normal;
	const values = new Map(
		(normal ?? []).map((value) => [
			`${value.fixtureId}:${value.attribute}`,
			value,
		]),
	);
	for (const value of pending)
		values.set(`${value.fixtureId}:${value.attribute}`, value);
	return [...values.values()];
}
