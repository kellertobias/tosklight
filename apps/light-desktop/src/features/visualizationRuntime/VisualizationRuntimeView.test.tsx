import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { VisualizationSnapshot } from "../../api/types";
import type {
	VisualizationRuntimeLane,
	VisualizationRuntimeScope,
} from "./contracts";
import { VisualizationRuntimeStore } from "./store";
import type { VisualizationRuntimeTransport } from "./transport";
import {
	useVisualizationRuntimeSnapshotSubscription,
	useVisualizationRuntimeView,
	VisualizationRuntimeProvider,
} from "./VisualizationRuntimeView";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const SESSION_ID = "22222222-2222-4222-8222-222222222222";

afterEach(() => {
	cleanup();
	TestBroadcastChannel.reset();
	vi.unstubAllGlobals();
	vi.useRealTimers();
});

describe("VisualizationRuntimeProvider", () => {
	it("opens no request or subscription for disabled views", async () => {
		const transport = fakeTransport();
		const store = new VisualizationRuntimeStore();
		const renders = vi.fn();
		render(
			provider(<Probe enabled={false} renders={renders} />, transport, {
				store,
			}),
		);

		expect(screen.getByText("normal:idle:—")).toBeInTheDocument();
		await act(async () => undefined);
		expect(transport.loadSnapshot).not.toHaveBeenCalled();
		const dormantRenders = renders.mock.calls.length;
		act(() => store.install("normal", snapshot("normal")));
		expect(renders).toHaveBeenCalledTimes(dormantRenders);
	});

	it("keeps preload updates out of a normal selector", async () => {
		const store = new VisualizationRuntimeStore();
		const transport = fakeTransport();
		const normalRenders = vi.fn();
		const preloadRenders = vi.fn();
		render(
			provider(
				<>
					<Probe renders={normalRenders} />
					<Probe lane="preload" renders={preloadRenders} />
				</>,
				transport,
				{ store },
			),
		);
		await waitFor(() =>
			expect(screen.getByText("normal:ready:1")).toBeInTheDocument(),
		);
		await waitFor(() =>
			expect(screen.getByText("preload:ready:1")).toBeInTheDocument(),
		);
		const normalReadyRenders = normalRenders.mock.calls.length;

		act(() =>
			store.install("preload", { ...snapshot("preload"), revision: 2 }),
		);

		expect(screen.getByText("preload:ready:2")).toBeInTheDocument();
		expect(normalRenders).toHaveBeenCalledTimes(normalReadyRenders);
		expect(preloadRenders.mock.calls.length).toBeGreaterThan(0);
	});

	it("shares one normal request across multiple mounted consumers", async () => {
		const transport = fakeTransport();
		render(
			provider(
				<>
					<Probe />
					<Probe />
					<Probe />
				</>,
				transport,
			),
		);

		await waitFor(() =>
			expect(screen.getAllByText("normal:ready:1")).toHaveLength(3),
		);
		expect(transport.loadSnapshot).toHaveBeenCalledOnce();
	});

	it("releases the final Live stream claim when only Preload remains mounted", async () => {
		const updateClaims = vi.fn();
		const transport = {
			loadSnapshot: vi.fn(),
			openStream: vi.fn(() => ({
				updateClaims,
				close: vi.fn(),
			})),
		} satisfies VisualizationRuntimeTransport;
		const rendered = render(
			provider(
				<>
					<Probe />
					<Probe lane="preload" />
				</>,
				transport,
			),
		);
		await waitFor(() =>
			expect(updateClaims).toHaveBeenLastCalledWith(["normal", "preload"], 4),
		);

		rendered.rerender(provider(<Probe lane="preload" />, transport));

		await waitFor(() =>
			expect(updateClaims).toHaveBeenLastCalledWith(["preload"], 4),
		);
	});

	it("delivers live frames imperatively without reconciling the observing component", async () => {
		const transport = fakeTransport();
		const store = new VisualizationRuntimeStore();
		const renders = vi.fn();
		const snapshots = vi.fn();
		render(
			provider(
				<ImperativeProbe renders={renders} snapshots={snapshots} />,
				transport,
				{ store },
			),
		);
		await waitFor(() =>
			expect(screen.getByText("normal:ready:1")).toBeInTheDocument(),
		);
		const readyRenders = renders.mock.calls.length;

		act(() => store.install("normal", { ...snapshot("normal"), revision: 2 }));

		expect(snapshots).toHaveBeenLastCalledWith(
			expect.objectContaining({ revision: 2 }),
		);
		expect(renders).toHaveBeenCalledTimes(readyRenders);
		expect(screen.getByText("normal:ready:1")).toBeInTheDocument();
	});

	it("clears immediately and drops a late response after server replacement", async () => {
		const first = deferred<VisualizationSnapshot>();
		const transportA = fakeTransport(() => first.promise);
		const transportB = fakeTransport(async (lane) => ({
			...snapshot(lane),
			revision: 2,
		}));
		const child = <Probe />;
		const rendered = render(provider(child, transportA));
		await waitFor(() => expect(transportA.loadSnapshot).toHaveBeenCalledOnce());

		rendered.rerender(
			provider(child, transportB, { authorityKey: "server-b" }),
		);
		expect(screen.getByText("normal:loading:—")).toBeInTheDocument();
		await waitFor(() =>
			expect(screen.getByText("normal:ready:2")).toBeInTheDocument(),
		);
		first.resolve({ ...snapshot("normal"), revision: 99 });
		await act(async () => first.promise);

		expect(screen.getByText("normal:ready:2")).toBeInTheDocument();
	});

	it("routes a secondary desktop Stage claim through the owner runtime", async () => {
		vi.stubGlobal("BroadcastChannel", TestBroadcastChannel);
		vi.stubGlobal("__TAURI_INTERNALS__", {});
		const ownerTransport = fakeTransport();
		const mirrorTransport = {
			...fakeTransport(),
			openStream: vi.fn(() => ({
				updateClaims: vi.fn(),
				close: vi.fn(),
			})),
		} satisfies VisualizationRuntimeTransport;
		render(
			<>
				<VisualizationRuntimeProvider
					showId={SHOW_ID}
					sessionId={SESSION_ID}
					authorityKey="server-a|generation-1"
					desktopAuthorityKey="server-a|shared-desk"
					transport={ownerTransport}
					desktopRole="owner"
				>
					<div />
				</VisualizationRuntimeProvider>
				<VisualizationRuntimeProvider
					showId={SHOW_ID}
					sessionId={SESSION_ID}
					authorityKey="server-a|generation-2"
					desktopAuthorityKey="server-a|shared-desk"
					transport={mirrorTransport}
					desktopRole="mirror"
				>
					<Probe />
				</VisualizationRuntimeProvider>
			</>,
		);

		await waitFor(() =>
			expect(screen.getByText("normal:ready:1")).toBeInTheDocument(),
		);
		expect(ownerTransport.loadSnapshot).toHaveBeenCalledOnce();
		expect(mirrorTransport.loadSnapshot).not.toHaveBeenCalled();
		expect(mirrorTransport.openStream).not.toHaveBeenCalled();
	});

	it("preserves the owner's stale error on a mirrored snapshot", async () => {
		vi.stubGlobal("BroadcastChannel", TestBroadcastChannel);
		vi.stubGlobal("__TAURI_INTERNALS__", {});
		const ownerStore = new VisualizationRuntimeStore();
		const mirrorStore = new VisualizationRuntimeStore();
		render(
			<>
				<VisualizationRuntimeProvider
					showId={SHOW_ID}
					sessionId={SESSION_ID}
					authorityKey="server-a|generation-1"
					desktopAuthorityKey="server-a|shared-desk"
					transport={fakeTransport()}
					store={ownerStore}
					desktopRole="owner"
				>
					<div />
				</VisualizationRuntimeProvider>
				<VisualizationRuntimeProvider
					showId={SHOW_ID}
					sessionId={SESSION_ID}
					authorityKey="server-a|generation-2"
					desktopAuthorityKey="server-a|shared-desk"
					transport={fakeTransport()}
					store={mirrorStore}
					desktopRole="mirror"
				>
					<Probe />
				</VisualizationRuntimeProvider>
			</>,
		);
		await waitFor(() =>
			expect(mirrorStore.getSnapshot().normal.snapshot).not.toBeNull(),
		);

		act(() => ownerStore.setError("normal", new Error("stream interrupted")));

		await waitFor(() =>
			expect(mirrorStore.getSnapshot().normal.error?.message).toBe(
				"stream interrupted",
			),
		);
		expect(mirrorStore.getSnapshot().normal.status).toBe("ready");
	});

	it("falls back to its own runtime when desktop bridge delivery never arrives", async () => {
		vi.useFakeTimers();
		vi.stubGlobal("BroadcastChannel", TestBroadcastChannel);
		vi.stubGlobal("__TAURI_INTERNALS__", {});
		const transport = fakeTransport();
		render(
			<VisualizationRuntimeProvider
				showId={SHOW_ID}
				sessionId={SESSION_ID}
				authorityKey="server-a|generation-2"
				desktopAuthorityKey="server-a|shared-desk"
				transport={transport}
				desktopRole="mirror"
			>
				<Probe />
			</VisualizationRuntimeProvider>,
		);

		await act(async () => {
			vi.advanceTimersByTime(6_000);
			await Promise.resolve();
		});

		expect(transport.loadSnapshot).toHaveBeenCalledOnce();
	});
});

function Probe({
	lane = "normal",
	enabled = true,
	renders,
}: {
	lane?: VisualizationRuntimeLane;
	enabled?: boolean;
	renders?: () => void;
}) {
	renders?.();
	const view = useVisualizationRuntimeView({
		lane,
		enabled,
		intervalMillis: 250,
	});
	return (
		<span>{`${lane}:${view.status}:${view.snapshot?.revision ?? "—"}`}</span>
	);
}

function ImperativeProbe({
	renders,
	snapshots,
}: {
	renders: () => void;
	snapshots: (snapshot: VisualizationSnapshot) => void;
}) {
	renders();
	const view = useVisualizationRuntimeView({
		lane: "normal",
		enabled: true,
		intervalMillis: 250,
		reconcileSnapshots: false,
	});
	useVisualizationRuntimeSnapshotSubscription("normal", true, snapshots);
	return (
		<span>{`normal:${view.status}:${view.snapshot?.revision ?? "—"}`}</span>
	);
}

function provider(
	child: ReactNode,
	transport: VisualizationRuntimeTransport,
	options: {
		store?: VisualizationRuntimeStore;
		authorityKey?: string;
	} = {},
) {
	return (
		<VisualizationRuntimeProvider
			showId={SHOW_ID}
			sessionId={SESSION_ID}
			authorityKey={options.authorityKey ?? "server-a"}
			transport={transport}
			store={options.store}
		>
			{child}
		</VisualizationRuntimeProvider>
	);
}

function fakeTransport(
	implementation: (
		lane: VisualizationRuntimeLane,
	) => Promise<VisualizationSnapshot> = async (lane) => snapshot(lane),
) {
	return {
		loadSnapshot: vi.fn(
			(_scope: VisualizationRuntimeScope, lane: VisualizationRuntimeLane) =>
				implementation(lane),
		),
	} satisfies VisualizationRuntimeTransport;
}

function snapshot(lane: VisualizationRuntimeLane): VisualizationSnapshot {
	return {
		revision: 1,
		generated_at: "2026-07-21T09:00:00Z",
		grand_master: 1,
		blackout: false,
		preload: lane === "preload",
		values: [],
		profile_output_values: [],
	};
}

function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((done) => {
		resolve = done;
	});
	return { promise, resolve };
}

class TestBroadcastChannel {
	static readonly instances = new Set<TestBroadcastChannel>();
	onmessage: ((event: MessageEvent) => void) | null = null;

	constructor(readonly name: string) {
		TestBroadcastChannel.instances.add(this);
	}

	postMessage(data: unknown) {
		for (const instance of TestBroadcastChannel.instances) {
			if (instance === this || instance.name !== this.name) continue;
			queueMicrotask(() => instance.onmessage?.({ data } as MessageEvent));
		}
	}

	close() {
		TestBroadcastChannel.instances.delete(this);
	}

	static reset() {
		TestBroadcastChannel.instances.clear();
	}
}
