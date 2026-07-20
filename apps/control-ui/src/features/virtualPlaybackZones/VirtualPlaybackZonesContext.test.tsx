import { act, render } from "@testing-library/react";
import { type ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";
import type {
	VirtualPlaybackZonesAuthority,
	VirtualPlaybackZonesCapability,
	VirtualPlaybackZonesSnapshot,
	VirtualPlaybackZonesTransport,
} from "./contracts";
import {
	useVirtualPlaybackZones,
	VirtualPlaybackZonesProvider,
} from "./VirtualPlaybackZonesContext";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const DESK_ID = "22222222-2222-4222-8222-222222222222";
const ZONES = [{ id: "paired", name: "Paired", slots: [1, 2] }] as const;
const UPDATED_ZONES = [
	{ id: "paired", name: "Updated", slots: [1, 2, 3] },
] as const;

function authority(authorityId: string): VirtualPlaybackZonesAuthority {
	return { authorityId, scope: { showId: SHOW_ID, deskId: DESK_ID } };
}

function snapshot(
	surfaces: VirtualPlaybackZonesSnapshot["surfaces"] = {},
): VirtualPlaybackZonesSnapshot {
	return { showId: SHOW_ID, deskId: DESK_ID, surfaces };
}

function deferred<T>() {
	let resolve!: (value: T) => void;
	let reject!: (reason: unknown) => void;
	const promise = new Promise<T>((accept, decline) => {
		resolve = accept;
		reject = decline;
	});
	return { promise, resolve, reject };
}

function fakeTransport(
	loadSnapshot: VirtualPlaybackZonesTransport["loadSnapshot"],
	saveSurface: VirtualPlaybackZonesTransport["saveSurface"] = vi.fn(),
): VirtualPlaybackZonesTransport {
	return { loadSnapshot, saveSurface };
}

function harness(
	current: { capability: VirtualPlaybackZonesCapability | null },
	selectedAuthority: VirtualPlaybackZonesAuthority | null,
	transport: VirtualPlaybackZonesTransport | null,
	child: ReactNode = null,
) {
	function Probe() {
		current.capability = useVirtualPlaybackZones();
		return <>{child}</>;
	}
	return (
		<VirtualPlaybackZonesProvider
			authority={selectedAuthority}
			transport={transport}
		>
			<Probe />
		</VirtualPlaybackZonesProvider>
	);
}

describe("VirtualPlaybackZonesProvider", () => {
	it("performs no read on mount and coalesces caller-triggered reads", async () => {
		const pending = deferred<VirtualPlaybackZonesSnapshot>();
		const loadSnapshot = vi.fn(() => pending.promise);
		const transport = fakeTransport(loadSnapshot);
		const current = { capability: null as VirtualPlaybackZonesCapability | null };
		render(harness(current, authority("session-a"), transport));

		expect(current.capability?.available).toBe(true);
		expect(loadSnapshot).not.toHaveBeenCalled();
		const first = current.capability?.loadSurface("surface-a");
		const second = current.capability?.loadSurface("surface-b");
		expect(loadSnapshot).toHaveBeenCalledOnce();
		expect(loadSnapshot).toHaveBeenCalledWith(
			{ showId: SHOW_ID, deskId: DESK_ID },
		);
		pending.resolve(snapshot({ "surface-a": ZONES }));
		await act(async () => {
			await expect(first).resolves.toEqual(ZONES);
			await expect(second).resolves.toEqual([]);
		});
	});

	it("saves a surface and exposes local failures", async () => {
		const saveSurface = vi
			.fn<VirtualPlaybackZonesTransport["saveSurface"]>()
			.mockResolvedValueOnce({ surfaceId: "surface-a", zones: ZONES })
			.mockRejectedValueOnce(new Error("save failed"));
		const transport = fakeTransport(vi.fn(), saveSurface);
		const current = { capability: null as VirtualPlaybackZonesCapability | null };
		render(harness(current, authority("session-a"), transport));

		await act(async () => {
			await expect(
				current.capability?.saveSurface("surface-a", ZONES),
			).resolves.toEqual(ZONES);
		});
		expect(saveSurface).toHaveBeenCalledWith(
			{ showId: SHOW_ID, deskId: DESK_ID },
			"surface-a",
			ZONES,
		);
		await act(async () => {
			await expect(
				current.capability?.saveSurface("surface-a", ZONES),
			).resolves.toBeNull();
		});
		expect(current.capability?.error).toBe("save failed");

		act(() => current.capability?.clearError());
		expect(current.capability?.error).toBeNull();
	});

	it("serializes saves so an older response cannot overwrite a newer intent", async () => {
		const first = deferred<{ surfaceId: string; zones: typeof ZONES }>();
		const newest = [
			{ id: "paired", name: "Newest", slots: [1, 2, 4] },
		] as const;
		const saveSurface = vi
			.fn<VirtualPlaybackZonesTransport["saveSurface"]>()
			.mockReturnValueOnce(first.promise)
			.mockResolvedValueOnce({ surfaceId: "surface-a", zones: newest });
		const current = { capability: null as VirtualPlaybackZonesCapability | null };
		render(
			harness(
				current,
				authority("session-a"),
				fakeTransport(vi.fn(), saveSurface),
			),
		);

		const older = current.capability?.saveSurface("surface-a", ZONES);
		const newer = current.capability?.saveSurface("surface-a", newest);
		await Promise.resolve();
		expect(saveSurface).toHaveBeenCalledOnce();

		first.resolve({ surfaceId: "surface-a", zones: ZONES });
		await act(async () => {
			await expect(older).resolves.toEqual(ZONES);
			await expect(newer).resolves.toEqual(newest);
		});
		expect(saveSurface).toHaveBeenCalledTimes(2);
		expect(current.capability?.getSurface("surface-a")).toEqual(newest);
	});

	it("keeps a completed save when an older coalesced snapshot arrives later", async () => {
		const pending = deferred<VirtualPlaybackZonesSnapshot>();
		const listener = vi.fn();
		const transport = fakeTransport(
			vi.fn(() => pending.promise),
			vi.fn(async () => ({
				surfaceId: "surface-a",
				zones: UPDATED_ZONES,
			})),
		);
		const current = { capability: null as VirtualPlaybackZonesCapability | null };
		render(harness(current, authority("session-a"), transport));
		const unsubscribe = current.capability?.subscribeSurface(
			"surface-a",
			listener,
		);
		const loading = current.capability?.loadSurface("surface-a");

		await act(async () => {
			await expect(
				current.capability?.saveSurface("surface-a", UPDATED_ZONES),
			).resolves.toEqual(UPDATED_ZONES);
		});
		expect(current.capability?.getSurface("surface-a")).toEqual(UPDATED_ZONES);
		pending.resolve(snapshot({ "surface-a": ZONES }));
		await act(async () => {
			await expect(loading).resolves.toEqual(UPDATED_ZONES);
		});
		expect(current.capability?.getSurface("surface-a")).toEqual(UPDATED_ZONES);
		expect(listener).toHaveBeenCalledTimes(3);
		unsubscribe?.();
	});

	it("rejects a foreign typed transport result as a local error", async () => {
		const transport = fakeTransport(
			vi.fn(async () => ({ ...snapshot(), deskId: SHOW_ID })),
		);
		const current = { capability: null as VirtualPlaybackZonesCapability | null };
		render(harness(current, authority("session-a"), transport));

		await act(async () => {
			await expect(
				current.capability?.loadSurface("surface-a"),
			).resolves.toBeNull();
		});
		expect(current.capability?.error).toContain("changed authority scope");
	});

	it("ignores a late response after same-show session replacement", async () => {
		const oldRequest = deferred<VirtualPlaybackZonesSnapshot>();
		const transport = fakeTransport(
			vi
				.fn<VirtualPlaybackZonesTransport["loadSnapshot"]>()
				.mockReturnValueOnce(oldRequest.promise)
				.mockResolvedValueOnce(snapshot({ "surface-a": ZONES })),
		);
		const current = { capability: null as VirtualPlaybackZonesCapability | null };
		const rendered = render(
			harness(current, authority("session-a"), transport),
		);
		const stale = current.capability?.loadSurface("surface-a");

		rendered.rerender(harness(current, authority("session-b"), transport));
		oldRequest.resolve(snapshot({ "surface-a": ZONES }));
		await act(async () => {
			await expect(stale).resolves.toBeNull();
		});
		expect(current.capability?.error).toBeNull();
		await act(async () => {
			await expect(
				current.capability?.loadSurface("surface-a"),
			).resolves.toEqual(ZONES);
		});
	});

	it("ignores late errors and replaces the server transport in the same scope", async () => {
		const oldRequest = deferred<VirtualPlaybackZonesSnapshot>();
		const oldTransport = fakeTransport(vi.fn(() => oldRequest.promise));
		const newTransport = fakeTransport(
			vi.fn(async () => snapshot({ "surface-a": ZONES })),
		);
		const current = { capability: null as VirtualPlaybackZonesCapability | null };
		const rendered = render(
			harness(current, authority("session-a"), oldTransport),
		);
		const stale = current.capability?.loadSurface("surface-a");

		rendered.rerender(
			harness(current, authority("session-a"), newTransport),
		);
		oldRequest.reject(new Error("old server failed"));
		await act(async () => {
			await expect(stale).resolves.toBeNull();
			await expect(
				current.capability?.loadSurface("surface-a"),
			).resolves.toEqual(ZONES);
		});
		expect(current.capability?.error).toBeNull();
	});

	it("ignores a late save outcome after authority replacement", async () => {
		const oldSave = deferred<{ surfaceId: string; zones: typeof ZONES }>();
		const saveSurface = vi
			.fn<VirtualPlaybackZonesTransport["saveSurface"]>()
			.mockReturnValueOnce(oldSave.promise);
		const transport = fakeTransport(vi.fn(), saveSurface);
		const current = { capability: null as VirtualPlaybackZonesCapability | null };
		const rendered = render(
			harness(current, authority("session-a"), transport),
		);
		const stale = current.capability?.saveSurface("surface-a", ZONES);

		rendered.rerender(harness(current, authority("session-b"), transport));
		oldSave.resolve({ surfaceId: "surface-a", zones: ZONES });
		await act(async () => {
			await expect(stale).resolves.toBeNull();
		});
		expect(current.capability?.error).toBeNull();
	});
});
