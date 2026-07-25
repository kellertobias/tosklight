import { describe, expect, it } from "vitest";
import type { ProgrammerState } from "../../api/types";
import { programmerValueCount } from "./programmerActivity";

const programmer = (values: unknown[] = [], groupValues: ProgrammerState["group_values"] = {}) => ({
  session_id: "session", user_id: "user", selected: [], command_line: "", connected: true,
  blind: false, preview: false, highlight: false, values, group_values: groupValues,
}) satisfies ProgrammerState;

describe("programmer activity", () => {
  it("counts fixture and group-relative values as recordable and clearable content", () => {
    expect(programmerValueCount(programmer())).toBe(0);
    expect(programmerValueCount(programmer([{}]))).toBe(1);
    expect(programmerValueCount(programmer([], { group: { intensity: { value: 1, changed_at: "now" }, pan: { value: 0, changed_at: "now" } } }))).toBe(2);
  });
});
