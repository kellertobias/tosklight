import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type {
	VirtualPlaybackZone,
	VirtualPlaybackZonesSnapshot,
	VirtualPlaybackZonesTransport,
} from "../../../features/virtualPlaybackZones/contracts";
import { VirtualPlaybackZonesProvider } from "../../../features/virtualPlaybackZones/VirtualPlaybackZonesContext";
import { useVirtualPlaybackSurfaceZones } from "./useVirtualPlaybackSurfaceZones";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const ZONES = [
	{ id: "paired", name: "Paired", playbackNumbers: [1001, 1301] },
] as const;
const UPDATED = [
	{ id: "paired", name: "Updated", playbackNumbers: [1001, 1301, 1601] },
] as const;

function snapshot(
	zones: readonly VirtualPlaybackZone[] = ZONES,
	revision = 4,
): VirtualPlaybackZonesSnapshot {
	return { showId: SHOW_ID, revision, zones };
}

function wrapper(transport: VirtualPlaybackZonesTransport) {
	return function Wrapper({ children }: { children: ReactNode }) {
		return (
			<VirtualPlaybackZonesProvider
				authority={{
					authorityId: "authority-a",
					scope: { showId: SHOW_ID },
				}}
				transport={transport}
			>
				{children}
			</VirtualPlaybackZonesProvider>
		);
	};
}

afterEach(cleanup);

describe("useVirtualPlaybackSurfaceZones", () => {
	it("projects the same show-global zones into different panes", async () => {
		const loadSnapshot = vi.fn(async () => snapshot());
		const transport = { loadSnapshot, save: vi.fn() };
		const first = renderHook(
			() =>
				useVirtualPlaybackSurfaceZones({
					surfaceId: "pane-a",
					active: true,
					authorityReady: true,
				}),
			{ wrapper: wrapper(transport) },
		);
		const second = renderHook(
			() =>
				useVirtualPlaybackSurfaceZones({
					surfaceId: "pane-b",
					active: true,
					authorityReady: true,
				}),
			{ wrapper: wrapper(transport) },
		);
		await waitFor(() => expect(first.result.current.ready).toBe(true));
		await waitFor(() => expect(second.result.current.ready).toBe(true));
		expect(first.result.current.zones).toEqual(ZONES);
		expect(second.result.current.zones).toEqual(ZONES);
	});

	it("persists the complete shared set without a pane or page operand", async () => {
		const save = vi.fn(async () => ({
			...snapshot(UPDATED, 5),
			requestId: "request-a",
			replayed: false,
			changed: true,
		}));
		const result = renderHook(
			() =>
				useVirtualPlaybackSurfaceZones({
					surfaceId: "pane-a",
					active: true,
					authorityReady: true,
					pageMode: { type: "pinned", page: 7 },
				}),
			{
				wrapper: wrapper({
					loadSnapshot: vi.fn(async () => snapshot()),
					save,
				}),
			},
		);
		await waitFor(() => expect(result.result.current.ready).toBe(true));
		await act(async () => {
			expect(await result.result.current.persist(UPDATED)).toBe(true);
		});
		expect(save).toHaveBeenCalledWith(
			{ showId: SHOW_ID },
			4,
			UPDATED,
			expect.any(String),
		);
		expect(result.result.current.zones).toEqual(UPDATED);
	});

	it("does not load before runtime authority is ready", () => {
		const loadSnapshot = vi.fn(async () => snapshot());
		const result = renderHook(
			() =>
				useVirtualPlaybackSurfaceZones({
					surfaceId: "pane-a",
					active: true,
					authorityReady: false,
				}),
			{ wrapper: wrapper({ loadSnapshot, save: vi.fn() }) },
		);
		expect(result.result.current.ready).toBe(false);
		expect(loadSnapshot).not.toHaveBeenCalled();
	});
});
