import type { ProgrammerState } from "../../api/types";

export function programmerValueCount(programmer: ProgrammerState | undefined) {
  const fixtureValues = programmer?.values.length ?? 0;
  const groupValues = Object.values(programmer?.group_values ?? {}).reduce(
    (count, attributes) => count + Object.keys(attributes).length,
    0,
  );
  return fixtureValues + groupValues;
}
