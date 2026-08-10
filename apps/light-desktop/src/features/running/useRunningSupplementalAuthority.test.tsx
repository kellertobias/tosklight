// @vitest-environment jsdom

import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { SupplementalRuntimeEvent } from "../../api/runtimeModels";
import { useRunningSupplementalAuthority } from "./useRunningSupplementalAuthority";

afterEach(cleanup);

describe("useRunningSupplementalAuthority", () => {
	it("hydrates once and follows ordered Macro and Timecode events without polling", async () => {
		let listener: ((event: SupplementalRuntimeEvent) => void) | undefined;
		const runtime = vi.fn().mockResolvedValue({
			desk_id: "desk-a",
			active: [macro("running")],
			recent: [],
		});
		const timecodes = vi.fn().mockResolvedValue([timecode(3, 30)]);
		const actions = {
			macros: { runtime, cancel: vi.fn() },
			timecodes: { runtime: timecodes, stop: vi.fn() },
			showObjects: { objects: vi.fn().mockResolvedValue([]) },
			events: {
				onEvent: vi.fn((next: (event: SupplementalRuntimeEvent) => void) => {
					listener = next;
					return vi.fn();
				}),
			},
		};
		const rendered = renderHook(() =>
			useRunningSupplementalAuthority(true, "show-a", actions),
		);
		await waitFor(() => expect(rendered.result.current.loading).toBe(false));
		expect(runtime).toHaveBeenCalledOnce();
		expect(timecodes).toHaveBeenCalledOnce();

		act(() => {
			listener?.({
				type: "timecode_runtime_changed",
				snapshot: timecode(2, 10),
			});
			listener?.({
				type: "timecode_runtime_changed",
				snapshot: timecode(4, 40),
			});
			listener?.({
				type: "macro_execution_changed",
				execution: macro("succeeded"),
			});
		});

		expect(rendered.result.current.timecodes[0]?.revision).toBe(4);
		expect(rendered.result.current.timecodes[0]?.frame).toBe(40);
		expect(rendered.result.current.macros[0]?.state).toBe("succeeded");
		expect(runtime).toHaveBeenCalledOnce();
		expect(timecodes).toHaveBeenCalledOnce();
	});
});

function timecode(revision: number, frame: number) {
	return {
		timecode_id: "timecode-a",
		revision,
		state: "playing" as const,
		frame,
		duration_frame: 100,
		audio_linked: false,
	};
}

function macro(state: "running" | "succeeded") {
	return {
		execution_id: "execution-a",
		macro_id: "macro-a",
		macro_number: 1,
		macro_name: "Macro 1",
		source_revision: 1,
		desk_id: "desk-a",
		user_id: "user-a",
		session_id: "session-a",
		state,
		trigger: { type: "pool" as const },
		started_at: "2026-08-10T18:00:00Z",
		finished_at: state === "succeeded" ? "2026-08-10T18:00:01Z" : undefined,
	};
}
