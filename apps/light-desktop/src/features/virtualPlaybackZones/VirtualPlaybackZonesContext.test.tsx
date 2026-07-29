import { act, cleanup, render, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type {
	VirtualPlaybackZone,
	VirtualPlaybackZonesCapability,
	VirtualPlaybackZonesEventObserver,
	VirtualPlaybackZonesSnapshot,
	VirtualPlaybackZonesTransport,
} from "./contracts";
import {
	useVirtualPlaybackZones,
	VirtualPlaybackZonesProvider,
} from "./VirtualPlaybackZonesContext";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const AUTHORITY = {
	authorityId: "authority-a",
	scope: { showId: SHOW_ID },
};
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

function harness(transport: VirtualPlaybackZonesTransport) {
	const current = { capability: null as VirtualPlaybackZonesCapability | null };
	function Probe() {
		current.capability = useVirtualPlaybackZones();
		return null;
	}
	render(
		<VirtualPlaybackZonesProvider authority={AUTHORITY} transport={transport}>
			<Probe />
		</VirtualPlaybackZonesProvider>,
	);
	return current;
}

afterEach(cleanup);

describe("VirtualPlaybackZonesProvider", () => {
	it("is dormant until explicitly loaded and coalesces reads", async () => {
		const loadSnapshot = vi.fn(async () => snapshot());
		const current = harness({ loadSnapshot, save: vi.fn() });
		expect(loadSnapshot).not.toHaveBeenCalled();
		await act(async () => {
			await Promise.all([
				current.capability?.load(),
				current.capability?.load(),
			]);
		});
		expect(loadSnapshot).toHaveBeenCalledOnce();
		expect(current.capability?.getZones()).toEqual(ZONES);
	});

	it("saves against the one show-level revision and installs the result", async () => {
		const save = vi.fn(async () => ({
			...snapshot(UPDATED, 5),
			requestId: "request-a",
			replayed: false,
			changed: true,
		}));
		const current = harness({
			loadSnapshot: vi.fn(async () => snapshot()),
			save,
		});
		await act(async () => {
			await current.capability?.load();
			await current.capability?.save(UPDATED);
		});
		expect(save).toHaveBeenCalledWith(
			{ showId: SHOW_ID },
			4,
			UPDATED,
			expect.any(String),
		);
		expect(current.capability?.getZones()).toEqual(UPDATED);
	});

	it("serializes edits so the second uses the first result revision", async () => {
		const save = vi
			.fn<VirtualPlaybackZonesTransport["save"]>()
			.mockResolvedValueOnce({
				...snapshot(UPDATED, 5),
				requestId: "one",
				replayed: false,
				changed: true,
			})
			.mockResolvedValueOnce({
				...snapshot(ZONES, 6),
				requestId: "two",
				replayed: false,
				changed: true,
			});
		const current = harness({
			loadSnapshot: vi.fn(async () => snapshot()),
			save,
		});
		await act(async () => {
			await Promise.all([
				current.capability?.save(UPDATED),
				current.capability?.save(ZONES),
			]);
		});
		expect(save).toHaveBeenCalledTimes(2);
		expect(save.mock.calls[0][1]).toBe(4);
		expect(save.mock.calls[1][1]).toBe(5);
	});

	it("reloads the shared snapshot when another desk publishes a revision", async () => {
		let observer: VirtualPlaybackZonesEventObserver | null = null;
		const loadSnapshot = vi
			.fn<() => Promise<VirtualPlaybackZonesSnapshot>>()
			.mockResolvedValueOnce(snapshot())
			.mockResolvedValueOnce(snapshot(UPDATED, 5));
		const current = harness({
			loadSnapshot,
			save: vi.fn(),
			subscribe: (_scope, next) => {
				observer = next;
				return { close: vi.fn() };
			},
		});
		await act(async () => {
			current.capability?.activate();
			await current.capability?.load();
		});
		act(() => observer?.changed({ showId: SHOW_ID, revision: 5 }));
		await waitFor(() =>
			expect(current.capability?.getZones()).toEqual(UPDATED),
		);
		expect(loadSnapshot).toHaveBeenCalledTimes(2);
	});

	it("clears cached zones when the authority changes", async () => {
		const transport = {
			loadSnapshot: vi.fn(async () => snapshot()),
			save: vi.fn(),
		};
		const current = {
			capability: null as VirtualPlaybackZonesCapability | null,
		};
		function Probe() {
			current.capability = useVirtualPlaybackZones();
			return null;
		}
		const view = render(
			<VirtualPlaybackZonesProvider authority={AUTHORITY} transport={transport}>
				<Probe />
			</VirtualPlaybackZonesProvider>,
		);
		await act(async () => void (await current.capability?.load()));
		view.rerender(
			<VirtualPlaybackZonesProvider
				authority={{
					authorityId: "authority-b",
					scope: { showId: "33333333-3333-4333-8333-333333333333" },
				}}
				transport={transport}
			>
				<Probe />
			</VirtualPlaybackZonesProvider>,
		);
		expect(current.capability?.getZones()).toBeNull();
	});
});
