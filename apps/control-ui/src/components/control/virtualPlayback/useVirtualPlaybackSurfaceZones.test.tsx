import {
	act,
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type {
	VirtualPlaybackZone,
	VirtualPlaybackZonesAuthority,
	VirtualPlaybackZonesEventObserver,
	VirtualPlaybackZonesSnapshot,
	VirtualPlaybackZonesTransport,
} from "../../../features/virtualPlaybackZones/contracts";
import { VirtualPlaybackZonesProvider } from "../../../features/virtualPlaybackZones/VirtualPlaybackZonesContext";
import { useVirtualPlaybackSurfaceZones } from "./useVirtualPlaybackSurfaceZones";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const NEXT_SHOW_ID = "33333333-3333-4333-8333-333333333333";
const DESK_ID = "22222222-2222-4222-8222-222222222222";
const INITIAL_ZONES = [
	{ id: "paired", name: "Paired", slots: [1, 2, 144] },
] as const;
const UPDATED_ZONES = [
	{ id: "paired", name: "Updated", slots: [1, 2, 3, 144] },
] as const;

function authority(showId = SHOW_ID): VirtualPlaybackZonesAuthority {
	return {
		authorityId: "session-a",
		scope: { showId, deskId: DESK_ID },
	};
}

function snapshot(
	zones: readonly VirtualPlaybackZone[],
	showId = SHOW_ID,
): VirtualPlaybackZonesSnapshot {
	return {
		showId,
		desks: { [DESK_ID]: { "surface-a": zones } },
	};
}

function saveOutcome(zones: readonly VirtualPlaybackZone[]) {
	return {
		requestId: "request-a",
		showId: SHOW_ID,
		deskId: DESK_ID,
		surfaceId: "surface-a",
		zones,
		replayed: false,
		changed: true,
	};
}

function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((accept) => {
		resolve = accept;
	});
	return { promise, resolve };
}

function SurfaceProbe({
	label,
	active = true,
	canSave = false,
}: {
	label: string;
	active?: boolean;
	canSave?: boolean;
}) {
	const surface = useVirtualPlaybackSurfaceZones({
		surfaceId: "surface-a",
		active,
		authorityReady: true,
	});
	return (
		<section aria-label={label}>
			<output data-testid={`${label}-zones`}>
				{surface.ready
					? surface.zones.map((zone) => zone.name).join(",")
					: "loading"}
			</output>
			{surface.error && <p role="alert">{surface.error}</p>}
			{canSave && (
				<button
					type="button"
					aria-label={`${label} save`}
					disabled={surface.saving}
					onClick={() => void surface.persist(UPDATED_ZONES)}
				>
					Save
				</button>
			)}
		</section>
	);
}

function tree(
	selectedAuthority: VirtualPlaybackZonesAuthority,
	transport: VirtualPlaybackZonesTransport,
	children: ReactNode,
) {
	return (
		<VirtualPlaybackZonesProvider
			authority={selectedAuthority}
			transport={transport}
		>
			{children}
		</VirtualPlaybackZonesProvider>
	);
}

afterEach(cleanup);

describe("useVirtualPlaybackSurfaceZones", () => {
	it("shares one snapshot and a saved surface across two consumers", async () => {
		const loadSnapshot = vi.fn(async () => snapshot(INITIAL_ZONES));
		const saveSurface = vi.fn(async () => saveOutcome(UPDATED_ZONES));
		const transport = { loadSnapshot, saveSurface };
		render(
			tree(
				authority(),
				transport,
				<>
					<SurfaceProbe label="pane" canSave />
					<SurfaceProbe label="settings" />
				</>,
			),
		);

		await waitFor(() => {
			expect(screen.getByTestId("pane-zones")).toHaveTextContent("Paired");
			expect(screen.getByTestId("settings-zones")).toHaveTextContent("Paired");
		});
		expect(loadSnapshot).toHaveBeenCalledOnce();

		fireEvent.click(screen.getByRole("button", { name: "pane save" }));
		await waitFor(() => {
			expect(screen.getByTestId("pane-zones")).toHaveTextContent("Updated");
			expect(screen.getByTestId("settings-zones")).toHaveTextContent(
				"Updated",
			);
		});
		expect(saveSurface).toHaveBeenCalledOnce();
		expect(loadSnapshot).toHaveBeenCalledOnce();
	});

	it("reloads an open matching surface after external invalidation and closes on inactivity", async () => {
		let observer: VirtualPlaybackZonesEventObserver | null = null;
		const close = vi.fn();
		const loadSnapshot = vi
			.fn<VirtualPlaybackZonesTransport["loadSnapshot"]>()
			.mockResolvedValueOnce(snapshot(INITIAL_ZONES))
			.mockResolvedValueOnce(snapshot(UPDATED_ZONES))
			.mockResolvedValueOnce(snapshot(INITIAL_ZONES))
			.mockResolvedValueOnce(snapshot(UPDATED_ZONES));
		const transport: VirtualPlaybackZonesTransport = {
			loadSnapshot,
			saveSurface: vi.fn(),
			subscribe: vi.fn((_scope, nextObserver) => {
				observer = nextObserver;
				return { close };
			}),
		};
		const rendered = render(
			tree(authority(), transport, <SurfaceProbe label="pane" />),
		);
		await waitFor(() =>
			expect(screen.getByTestId("pane-zones")).toHaveTextContent("Paired"),
		);

		act(() =>
			observer?.changed({
				showId: SHOW_ID,
				deskId: DESK_ID,
				surfaceId: "surface-a",
			}),
		);
		await waitFor(() =>
			expect(screen.getByTestId("pane-zones")).toHaveTextContent("Updated"),
		);
		expect(loadSnapshot).toHaveBeenCalledTimes(2);
		act(() => observer?.gap());
		await waitFor(() =>
			expect(screen.getByTestId("pane-zones")).toHaveTextContent("Paired"),
		);
		expect(loadSnapshot).toHaveBeenCalledTimes(3);
		act(() => observer?.error(new Error("zone events failed")));
		expect(screen.getByRole("alert")).toHaveTextContent("zone events failed");

		rendered.rerender(
			tree(
				authority(),
				transport,
				<SurfaceProbe label="pane" active={false} />,
			),
		);
		expect(close).toHaveBeenCalledOnce();
		act(() =>
			observer?.changed({
				showId: SHOW_ID,
				deskId: DESK_ID,
				surfaceId: "surface-a",
			}),
		);
		expect(loadSnapshot).toHaveBeenCalledTimes(3);

		rendered.rerender(
			tree(authority(), transport, <SurfaceProbe label="pane" />),
		);
		await waitFor(() => expect(loadSnapshot).toHaveBeenCalledTimes(4));
	});

	it("blocks overlapping edits while one surface save is pending", async () => {
		const pending = deferred<ReturnType<typeof saveOutcome>>();
		const saveSurface = vi.fn(() => pending.promise);
		render(
			tree(
				authority(),
				{
					loadSnapshot: vi.fn(async () => snapshot(INITIAL_ZONES)),
					saveSurface,
				},
				<>
					<SurfaceProbe label="pane" canSave />
					<SurfaceProbe label="settings" canSave />
				</>,
			),
		);
		await waitFor(() =>
			expect(screen.getByTestId("pane-zones")).toHaveTextContent("Paired"),
		);

		const paneSave = screen.getByRole("button", { name: "pane save" });
		const settingsSave = screen.getByRole("button", { name: "settings save" });
		fireEvent.click(paneSave);
		expect(paneSave).toBeDisabled();
		expect(settingsSave).toBeDisabled();
		await waitFor(() => expect(saveSurface).toHaveBeenCalledOnce());
		fireEvent.click(settingsSave);
		expect(saveSurface).toHaveBeenCalledOnce();

		pending.resolve(saveOutcome(UPDATED_ZONES));
		await waitFor(() => {
			expect(paneSave).toBeEnabled();
			expect(settingsSave).toBeEnabled();
		});
		expect(screen.getByTestId("pane-zones")).toHaveTextContent("Updated");
	});

	it("stays dormant until a consumer becomes active", async () => {
		const loadSnapshot = vi.fn(async () => snapshot(INITIAL_ZONES));
		const transport = { loadSnapshot, saveSurface: vi.fn() };
		const rendered = render(
			tree(authority(), transport, <SurfaceProbe label="pane" active={false} />),
		);

		expect(loadSnapshot).not.toHaveBeenCalled();
		rendered.rerender(
			tree(authority(), transport, <SurfaceProbe label="pane" />),
		);
		await waitFor(() =>
			expect(screen.getByTestId("pane-zones")).toHaveTextContent("Paired"),
		);
		expect(loadSnapshot).toHaveBeenCalledOnce();
	});

	it("does not reload merely because a local load error is reported", async () => {
		const loadSnapshot = vi.fn(async () => {
			throw new Error("load failed");
		});
		const transport = { loadSnapshot, saveSurface: vi.fn() };
		render(tree(authority(), transport, <SurfaceProbe label="pane" />));

		await waitFor(() =>
			expect(screen.getByRole("alert")).toHaveTextContent("load failed"),
		);
		expect(loadSnapshot).toHaveBeenCalledOnce();
	});

	it("replaces same-session show cache and ignores the old late snapshot", async () => {
		const oldSnapshot = deferred<VirtualPlaybackZonesSnapshot>();
		const loadSnapshot = vi
			.fn<VirtualPlaybackZonesTransport["loadSnapshot"]>()
			.mockReturnValueOnce(oldSnapshot.promise)
			.mockResolvedValueOnce(snapshot(UPDATED_ZONES, NEXT_SHOW_ID));
		const transport = { loadSnapshot, saveSurface: vi.fn() };
		const rendered = render(
			tree(authority(), transport, <SurfaceProbe label="pane" />),
		);
		await waitFor(() => expect(loadSnapshot).toHaveBeenCalledOnce());

		rendered.rerender(
			tree(
				authority(NEXT_SHOW_ID),
				transport,
				<SurfaceProbe label="pane" />,
			),
		);
		await waitFor(() =>
			expect(screen.getByTestId("pane-zones")).toHaveTextContent("Updated"),
		);

		oldSnapshot.resolve(snapshot(INITIAL_ZONES));
		await act(async () => {
			await oldSnapshot.promise;
		});
		expect(screen.getByTestId("pane-zones")).toHaveTextContent("Updated");
		expect(loadSnapshot).toHaveBeenCalledTimes(2);
	});
});
