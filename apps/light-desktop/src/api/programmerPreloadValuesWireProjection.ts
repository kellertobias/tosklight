import type { ProgrammerPreloadValuesProjection } from "../features/programmerPreloadValues/contracts";
import {
	decodeProgrammerValuesProjection,
	programmerValuesUuidAt,
} from "./programmerValuesWireProjection";

/** The two value authorities deliberately share one strict value-shape decoder. */
export function decodeProgrammerPreloadValuesProjection(
	value: unknown,
	path: string,
): ProgrammerPreloadValuesProjection {
	return decodeProgrammerValuesProjection(value, path);
}

export const programmerPreloadValuesUuidAt = programmerValuesUuidAt;
