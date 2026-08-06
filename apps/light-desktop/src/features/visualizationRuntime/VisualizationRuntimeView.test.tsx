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
import type { VisualizationSnapshot } from "../../api/types";
import { frontendPerformanceDiagnostics } from "../frontendWarmup/diagnostics";
import type {
	VisualizationRuntimeLane,
	VisualizationRuntimeScope,
} from "./contracts";
import { VisualizationRuntimeStore } from "./store";
import type {
	VisualizationRuntimeStreamObserver,
	VisualizationRuntimeTransport,
} from "./transport";
import {
	useDesktopVisualizationRuntimeRenderAcknowledgement,
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
			expect(updateClaims).toHaveBeenLastCalledWith(
				["normal", "preload"],
				4,
				false,
			),
		);

		rendered.rerender(provider(<Probe lane="preload" />, transport));

		await waitFor(() =>
			expect(updateClaims).toHaveBeenLastCalledWith(["preload"], 4, false),
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

	it("opens the secondary desktop Stage stream directly without cloning owner state", async () => {
		vi.stubGlobal("BroadcastChannel", TestBroadcastChannel);
		vi.stubGlobal("__TAURI_INTERNALS__", {});
		let observer: VisualizationRuntimeStreamObserver | null = null;
		const updateClaims = vi.fn();
		const mirrorTransport = {
			...fakeTransport(),
			openStream: vi.fn((_scope, nextObserver) => {
				observer = nextObserver;
				return {
					updateClaims,
					close: vi.fn(),
				};
			}),
		} satisfies VisualizationRuntimeTransport;
		render(
			<>
				<VisualizationRuntimeProvider
					showId={SHOW_ID}
					sessionId={SESSION_ID}
					authorityKey="server-a|generation-1"
					desktopAuthorityKey="server-a|shared-desk"
					transport={fakeTransport()}
					desktopRole="owner"
				>
					<div />
				</VisualizationRuntimeProvider>
				<VisualizationRuntimeProvider
					showId={SHOW_ID}
					sessionId="33333333-3333-4333-8333-333333333333"
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
			expect(mirrorTransport.openStream).toHaveBeenCalledOnce(),
		);
		expect(updateClaims).toHaveBeenLastCalledWith(["normal"], 4, false);
		act(() => observer?.snapshot("normal", snapshot("normal")));
		await waitFor(() =>
			expect(screen.getByText("normal:ready:1")).toBeInTheDocument(),
		);
		// One authoritative read per newly claimed lane is the deliberate bootstrap: a stream may
		// legally stay silent until the first change, so without it a secondary window waits
		// forever for an initial snapshot. It is a direct read from this window's own transport,
		// which is the opposite of cloning the owner's state — that is what the assertion below
		// pins, and it is the part that must never regress.
		expect(mirrorTransport.loadSnapshot).toHaveBeenCalledTimes(1);
		expect(
			TestBroadcastChannel.messages.some(
				(message) =>
					typeof message === "object" &&
					message !== null &&
					"type" in message &&
					(message.type === "state" || message.type === "claim"),
			),
		).toBe(false);
	});

	it("restarts the direct secondary stream at a replacement scope", async () => {
		vi.stubGlobal("BroadcastChannel", TestBroadcastChannel);
		vi.stubGlobal("__TAURI_INTERNALS__", {});
		const closeA = vi.fn();
		const transportA = {
			...fakeTransport(),
			openStream: vi.fn(() => ({
				updateClaims: vi.fn(),
				close: closeA,
			})),
		} satisfies VisualizationRuntimeTransport;
		const transportB = {
			...fakeTransport(),
			openStream: vi.fn(() => ({
				updateClaims: vi.fn(),
				close: vi.fn(),
			})),
		} satisfies VisualizationRuntimeTransport;
		const view = (showId: string, transport: VisualizationRuntimeTransport) => (
			<VisualizationRuntimeProvider
				showId={showId}
				sessionId={SESSION_ID}
				authorityKey={`server-a|${showId}`}
				desktopAuthorityKey="server-a|shared-desk"
				transport={transport}
				desktopRole="mirror"
			>
				<Probe />
			</VisualizationRuntimeProvider>
		);
		const rendered = render(view(SHOW_ID, transportA));
		await waitFor(() => expect(transportA.openStream).toHaveBeenCalledOnce());

		rendered.rerender(view("44444444-4444-4444-8444-444444444444", transportB));

		await waitFor(() => expect(transportB.openStream).toHaveBeenCalledOnce());
		await waitFor(() => expect(closeA).toHaveBeenCalled());
	});

	it("broadcasts only lightweight owner authority heartbeats", async () => {
		vi.stubGlobal("BroadcastChannel", TestBroadcastChannel);
		vi.stubGlobal("__TAURI_INTERNALS__", {});
		render(
			<VisualizationRuntimeProvider
				showId={SHOW_ID}
				sessionId={SESSION_ID}
				authorityKey="server-a|generation-1"
				desktopAuthorityKey="server-a|shared-desk"
				transport={fakeTransport()}
				desktopRole="owner"
			>
				<div />
			</VisualizationRuntimeProvider>,
		);

		await waitFor(() =>
			expect(TestBroadcastChannel.messages).toContainEqual(
				expect.objectContaining({
					type: "owner-heartbeat",
					showId: SHOW_ID,
					authorityKey: "server-a|shared-desk",
				}),
			),
		);
		expect(
			TestBroadcastChannel.messages.some(
				(message) =>
					typeof message === "object" &&
					message !== null &&
					"type" in message &&
					message.type === "state",
			),
		).toBe(false);
	});

	it("keeps the lightweight secondary-window render acknowledgement", async () => {
		vi.stubGlobal("BroadcastChannel", TestBroadcastChannel);
		vi.stubGlobal("__TAURI_INTERNALS__", {});
		const record = vi.spyOn(
			frontendPerformanceDiagnostics,
			"recordStageDesktopMirrorRender",
		);
		render(
			<>
				<VisualizationRuntimeProvider
					showId={SHOW_ID}
					sessionId={SESSION_ID}
					authorityKey="server-a|generation-1"
					desktopAuthorityKey="server-a|shared-desk"
					transport={fakeTransport()}
					desktopRole="owner"
				>
					<div />
				</VisualizationRuntimeProvider>
				<VisualizationRuntimeProvider
					showId={SHOW_ID}
					sessionId="33333333-3333-4333-8333-333333333333"
					authorityKey="server-a|generation-2"
					desktopAuthorityKey="server-a|shared-desk"
					transport={fakeTransport()}
					desktopRole="mirror"
				>
					<RenderAckProbe />
				</VisualizationRuntimeProvider>
			</>,
		);

		fireEvent.click(screen.getByRole("button", { name: "Acknowledge render" }));
		await waitFor(() => expect(record).toHaveBeenCalledOnce());
	});
});

function RenderAckProbe() {
	const acknowledge = useDesktopVisualizationRuntimeRenderAcknowledgement();
	return (
		<button type="button" onClick={() => acknowledge?.()}>
			Acknowledge render
		</button>
	);
}

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
	static readonly messages: unknown[] = [];
	onmessage: ((event: MessageEvent) => void) | null = null;

	constructor(readonly name: string) {
		TestBroadcastChannel.instances.add(this);
	}

	postMessage(data: unknown) {
		TestBroadcastChannel.messages.push(data);
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
		TestBroadcastChannel.messages.length = 0;
	}
}
