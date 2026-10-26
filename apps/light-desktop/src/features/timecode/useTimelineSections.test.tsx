// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type { TimecodeDefinition } from "../../api/types/timecode";
import { useTimelineSections } from "./useTimelineSections";

afterEach(cleanup);
it("expands Speed only through its explicit lane action, independently of point selection", () => {
 const definition = { id: "timeline", lanes: [{ id: "speed", content: { kind: "speed_group", group: "A", keyframes: [] } }] } as unknown as TimecodeDefinition;
 const props = { definition, activeLaneId: null as string | null, items: [], setSelection: vi.fn(), setSelectedLaneId: vi.fn(), setSelectedCueId: vi.fn(), prepareInspectorClose: vi.fn() };
 const hook = renderHook((input) => useTimelineSections(input), { initialProps: props });
 hook.rerender({ ...props, activeLaneId: "speed" });
 expect(hook.result.current.expandedSpeedLaneId).toBeNull();
 act(() => hook.result.current.onSelectLane("speed"));
 expect(hook.result.current.expandedSpeedLaneId).toBe("speed");
 act(() => hook.result.current.onCloseCue());
 expect(hook.result.current.expandedSpeedLaneId).toBeNull();
 hook.rerender({ ...props, activeLaneId: "speed" });
 expect(hook.result.current.expandedSpeedLaneId).toBeNull();
});
