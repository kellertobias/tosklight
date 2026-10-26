// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useTimelineInspectorVisibility } from "./useTimelineInspectorVisibility";
import { TimecodePaneScope, useTimecodeSidebarPreference } from "./useTimecodeSidebarPreference";
import { DeskConnectionProvider } from "../deskConnection/DeskConnectionContext";
import type { PropsWithChildren } from "react";

beforeEach(() => {
 const values = new Map<string, string>();
 vi.stubGlobal("localStorage", { getItem: (key: string) => values.get(key) ?? null, setItem: (key: string, value: string) => values.set(key, value) });
});
afterEach(() => { cleanup(); vi.unstubAllGlobals(); });
describe("optional Timecode sidebar presentation", () => {
 it("defaults off, persists locally, and isolates desk/window identity", () => {
  function wrapper({ children }: PropsWithChildren) {
   return <DeskConnectionProvider connection={{ deskLayoutScope: "desk-A", deskLayout: null, setServerUrl: vi.fn(), setDeskToken: vi.fn(), saveDeskLayout: vi.fn() }}><TimecodePaneScope.Provider value="pane-A">{children}</TimecodePaneScope.Provider></DeskConnectionProvider>;
  }
  const first = renderHook(useTimecodeSidebarPreference, { wrapper });
  expect(first.result.current[0]).toBe(false);
  act(() => first.result.current[1](true));
  first.unmount();
  const reopened = renderHook(useTimecodeSidebarPreference, { wrapper });
  expect(reopened.result.current[0]).toBe(true);
  const other = renderHook(useTimecodeSidebarPreference);
  expect(other.result.current[0]).toBe(false);
 });
 it("closes independently, reserves geometry first, and reopens only on explicit preference change", () => {
  const events: string[] = [];
  const prepare = vi.fn(() => events.push("reserve"));
  const change = vi.fn(() => events.push("preference"));
  const hook = renderHook(({ requested }) => useTimelineInspectorVisibility(requested, change, prepare), { initialProps: { requested: false } });
  expect(hook.result.current.visible).toBe(false);
  hook.rerender({ requested: true });
  expect(hook.result.current.visible).toBe(true);
  act(() => hook.result.current.close());
  expect(events).toEqual(["reserve", "preference"]);
  expect(hook.result.current.visible).toBe(false);
  hook.rerender({ requested: true });
  expect(hook.result.current.visible).toBe(false);
  hook.rerender({ requested: false });
  hook.rerender({ requested: true });
  expect(hook.result.current.visible).toBe(true);
  hook.rerender({ requested: false });
  expect(hook.result.current.visible).toBe(false);
  expect(prepare).toHaveBeenCalledTimes(2);
 });
});
