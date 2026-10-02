import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";
import type { OutputReadoutSnapshot } from "../../api/familyEncoderModels";
import { DisplayedSourceReadouts } from "../programmerValues/displayedSource";
import {
	type FamilyGestureIntentInput,
	FamilyGestureSession,
} from "../programmerValues/familyGestureSession";
import {
	POSITION_GESTURE_FAMILY,
	positionAngleStep,
} from "../programmerValues/positionGestureSession";
import type { VisualizationRuntimeSession } from "../visualizationRuntime/session";
import {
	FamilyEncodersContextProvider,
	type FamilyEncodersContextValue,
} from "./FamilyEncodersProvider";
import { useFamilyReadouts } from "./useFamilyReadouts";

const SHOW = "11111111-1111-4111-8111-111111111111";

function snapshot(
	lane: "normal" | "preload",
	lease: number,
	owners: readonly string[] = [],
): OutputReadoutSnapshot {
	return {
		lane,
		scope: { show_id: SHOW },
		lease,
		revision: 1,
		owners: owners.map((fixture_id) => ({
			fixture_id,
			position: { available: false, commands: [] },
		})),
	} as unknown as OutputReadoutSnapshot;
}

function rig() {
	const request = vi.fn(async (path: string) =>
		snapshot(path.includes("lane=preload") ? "preload" : "normal", 1),
	);
	const release = vi.fn();
	let listener: ((value: OutputReadoutSnapshot) => void) | null = null;
	const session = {
		claimReadouts: vi.fn((_ids: readonly string[], next: typeof listener) => {
			listener = next;
			return release;
		}),
	} as unknown as VisualizationRuntimeSession;
	const value: FamilyEncodersContextValue = {
		loadPages: vi.fn(),
		readouts: new DisplayedSourceReadouts({ request, showId: () => SHOW }),
		session,
	};
	const wrapper = ({ children }: { children: ReactNode }) => (
		<FamilyEncodersContextProvider value={value}>{children}</FamilyEncodersContextProvider>
	);
	return { request, release, session, wrapper, push: (next: OutputReadoutSnapshot) => listener?.(next) };
}

describe("useFamilyReadouts", () => {
	it("reads once, claims the stream on Normal and names the newest lease", async () => {
		const { request, session, wrapper, push } = rig();
		const hook = renderHook(() => useFamilyReadouts("normal", ["a", "b"]), { wrapper });
		await waitFor(() => expect(hook.result.current.snapshot?.lease).toBe(1));
		expect(request).toHaveBeenCalledOnce();
		expect(request.mock.calls[0]?.[0]).toBe(
			"/api/v2/output/readouts?lane=normal&fixture_ids=a,b",
		);
		expect(session.claimReadouts).toHaveBeenCalledWith(
			["a", "b"],
			expect.any(Function),
			"family-readouts",
		);
		act(() => push(snapshot("normal", 5)));
		expect(hook.result.current.displayedSource()).toEqual({ lane: "normal", lease: 5 });
	});

	it("reads Preload over HTTP only and never claims the Live stream", async () => {
		const { request, session, wrapper } = rig();
		const hook = renderHook(() => useFamilyReadouts("preload", ["a"]), { wrapper });
		await waitFor(() => expect(hook.result.current.snapshot?.lane).toBe("preload"));
		expect(session.claimReadouts).not.toHaveBeenCalled();
		expect(request.mock.calls[0]?.[0]).toContain("lane=preload");
	});

	it("releases the claim when disabled, hidden or unmounted", async () => {
		const { release, session, wrapper } = rig();
		const hook = renderHook(
			({ enabled }) => useFamilyReadouts("normal", ["a"], { enabled }),
			{ wrapper, initialProps: { enabled: true } },
		);
		expect(session.claimReadouts).toHaveBeenCalledOnce();
		hook.rerender({ enabled: false });
		expect(release).toHaveBeenCalledOnce();
		expect(hook.result.current.snapshot).toBeNull();

		hook.rerender({ enabled: true });
		const visibility = vi.spyOn(document, "visibilityState", "get").mockReturnValue("hidden");
		act(() => {
			document.dispatchEvent(new Event("visibilitychange"));
		});
		expect(release).toHaveBeenCalledTimes(2);
		visibility.mockRestore();
		hook.unmount();
	});

	it("another consumer's newer read never hides this consumer's readouts or lease", async () => {
		const answers = new Map<string, (value: unknown) => void>();
		const request = vi.fn(
			(path: string) =>
				new Promise<unknown>((resolve) => {
					answers.set(path, resolve);
				}),
		);
		const value: FamilyEncodersContextValue = {
			loadPages: vi.fn(),
			readouts: new DisplayedSourceReadouts({ request, showId: () => SHOW }),
			session: null as unknown as VisualizationRuntimeSession,
		};
		const wrapper = ({ children }: { children: ReactNode }) => (
			<FamilyEncodersContextProvider value={value}>{children}</FamilyEncodersContextProvider>
		);
		const encoder = renderHook(() => useFamilyReadouts("preload", ["a"]), { wrapper });
		const modal = renderHook(() => useFamilyReadouts("preload", ["b"]), { wrapper });
		await act(async () => {
			answers.get("/api/v2/output/readouts?lane=preload&fixture_ids=b")?.(
				snapshot("preload", 4, ["b"]),
			);
		});
		await act(async () => {
			answers.get("/api/v2/output/readouts?lane=preload&fixture_ids=a")?.(
				snapshot("preload", 3, ["a"]),
			);
		});
		expect(encoder.result.current.snapshot?.lease).toBe(3);
		expect(encoder.result.current.owner("a")).not.toBeNull();
		expect(encoder.result.current.displayedSource()).toEqual({ lane: "preload", lease: 3 });
		expect(modal.result.current.displayedSource()).toEqual({ lane: "preload", lease: 4 });
	});

	it("a gesture keeps the lease it read at its start while another consumer reads and rereads", async () => {
		const { wrapper, push } = rig();
		const encoder = renderHook(() => useFamilyReadouts("normal", ["a"]), { wrapper });
		const modal = renderHook(
			() => useFamilyReadouts("normal", ["b"], { consumerId: "modal" }),
			{ wrapper },
		);
		await waitFor(() => expect(encoder.result.current.snapshot?.lease).toBe(1));
		const applied: FamilyGestureIntentInput[] = [];
		const gestures = new FamilyGestureSession(POSITION_GESTURE_FAMILY, {
			writerFor: () => ({
				applyIntent: vi.fn(async (input: FamilyGestureIntentInput) => {
					applied.push(input);
					return { status: "changed" };
				}),
				cancelGesture: vi.fn(() => 0),
				finishGesture: vi.fn(async () => ({ status: "no_change" })),
			}),
			displayedSource: () => encoder.result.current.displayedSource(),
		});
		const gesture = gestures.start({
			lane: "normal",
			fixtureIds: ["a"],
			timing: { fade: false, fadeMillis: null, delayMillis: null },
		});
		await gesture?.change({ pan: positionAngleStep(1) });
		act(() => push(snapshot("normal", 9, ["a", "b"])));
		act(() => modal.result.current.reread());
		await gesture?.change({ pan: positionAngleStep(1) });
		expect(applied.map((input) => input.displayedSource)).toEqual([
			{ lane: "normal", lease: 1 },
			{ lane: "normal", lease: 1 },
		]);
	});
});
