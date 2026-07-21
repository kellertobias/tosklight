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
import type { OutputRuntimeActionOutcome } from "./contracts";
import {
	OutputRuntimeProvider,
	useOutputRuntimeActions,
	useOutputRuntimeBlackout,
	useOutputRuntimeView,
} from "./OutputRuntimeView";
import { OutputRuntimeStore } from "./store";
import {
	changedOutcome,
	deferred,
	DESK_ID,
	FakeOutputRuntimeTransport,
	outputProjection,
	settleOutputSession,
	SHOW_ID,
} from "./testFixtures";

afterEach(cleanup);

function DisabledProbe() {
	const view = useOutputRuntimeView(false);
	const actions = useOutputRuntimeActions(false);
	return <span>{`${view.status}:${String(actions)}`}</span>;
}

function ViewProbe() {
	const view = useOutputRuntimeView();
	return (
		<span>
			{view.ready
				? `${view.projection?.grandMaster}:${view.projection?.blackout}:${view.pending}`
				: "Loading"}
		</span>
	);
}

function ActionProbe({
	onOutcome,
}: {
	onOutcome: (outcome: OutputRuntimeActionOutcome | null) => void;
}) {
	const actions = useOutputRuntimeActions();
	return (
		<button
			type="button"
			onClick={() =>
				void actions
					?.setOutput({
						grandMaster: 0.4,
						blackout: true,
						requestId: "late-response",
					})
					.then(onOutcome)
			}
		>
			Set output
		</button>
	);
}

function provider(
	child: ReactNode,
	store: OutputRuntimeStore,
	transport: FakeOutputRuntimeTransport,
	authorityKey = "session-a",
) {
	return (
		<OutputRuntimeProvider
			showId={SHOW_ID}
			deskId={DESK_ID}
			authorityKey={authorityKey}
			store={store}
			transport={transport}
		>
			{child}
		</OutputRuntimeProvider>
	);
}

describe("OutputRuntimeProvider", () => {
	it("mounts dormant until an enabled view or action owner appears", async () => {
		const store = new OutputRuntimeStore();
		const transport = new FakeOutputRuntimeTransport();
		const subscribe = vi.spyOn(store, "subscribe");
		const rendered = render(provider(<DisabledProbe />, store, transport));

		expect(screen.getByText("idle:null")).toBeInTheDocument();
		await settleOutputSession();
		expect(subscribe).not.toHaveBeenCalled();
		expect(transport.loadSnapshot).not.toHaveBeenCalled();
		expect(transport.subscribe).not.toHaveBeenCalled();
		expect(transport.applyAction).not.toHaveBeenCalled();

		rendered.rerender(provider(<ViewProbe />, store, transport));
		await waitFor(() =>
			expect(screen.getByText("1:false:false")).toBeInTheDocument(),
		);
		expect(transport.loadSnapshot).toHaveBeenCalledOnce();
		expect(transport.subscribe).toHaveBeenCalledOnce();
	});

	it("drops a late mutation response after server authority replacement", async () => {
		const store = new OutputRuntimeStore();
		const transport = new FakeOutputRuntimeTransport();
		const response = deferred<OutputRuntimeActionOutcome>();
		transport.applyAction.mockReturnValueOnce(response.promise);
		const onOutcome = vi.fn();
		const child = (
			<>
				<ViewProbe />
				<ActionProbe onOutcome={onOutcome} />
			</>
		);
		const rendered = render(provider(child, store, transport));
		await waitFor(() =>
			expect(screen.getByText("1:false:false")).toBeInTheDocument(),
		);

		fireEvent.click(screen.getByRole("button", { name: "Set output" }));
		await waitFor(() =>
			expect(screen.getByText("0.4:true:true")).toBeInTheDocument(),
		);
		const request = transport.applyAction.mock.calls[0]?.[1];
		if (!request) throw new Error("missing Output request");

		rendered.rerender(provider(child, store, transport, "session-b"));
		await waitFor(() =>
			expect(screen.getByText("1:false:false")).toBeInTheDocument(),
		);
		response.resolve(
			changedOutcome(
				request.requestId,
				outputProjection({
					revision: 2,
					grandMaster: 0.4,
					blackout: true,
				}),
			),
		);
		await act(async () => response.promise);

		expect(onOutcome).toHaveBeenCalledWith(null);
		expect(store.getSnapshot()).toMatchObject({
			authorityKey: "session-b",
			authorityRevision: 1,
			projection: { grandMaster: 1, blackout: false },
		});
	});

	it("does not rerender unrelated children for scoped output events", async () => {
		const store = new OutputRuntimeStore();
		const transport = new FakeOutputRuntimeTransport();
		const onSiblingRender = vi.fn();
		function Sibling() {
			onSiblingRender();
			return <span>Unrelated</span>;
		}
		render(
			provider(
				<>
					<ViewProbe />
					<Sibling />
				</>,
				store,
				transport,
			),
		);
		await waitFor(() =>
			expect(screen.getByText("1:false:false")).toBeInTheDocument(),
		);
		const renders = onSiblingRender.mock.calls.length;

		act(() =>
			transport.emit({
				type: "event",
				sequence: 11,
				correlationId: null,
				change: {
					projection: outputProjection({
						revision: 2,
						grandMaster: 0.6,
					}),
				},
			}),
		);

		expect(screen.getByText("0.6:false:false")).toBeInTheDocument();
		expect(onSiblingRender).toHaveBeenCalledTimes(renders);
	});

	it("keeps a blackout scalar subscriber stable for Grand Master-only events", async () => {
		const store = new OutputRuntimeStore();
		const transport = new FakeOutputRuntimeTransport();
		const renders = vi.fn();
		function BlackoutProbe() {
			renders();
			return <span>Blackout {String(useOutputRuntimeBlackout())}</span>;
		}
		render(provider(<BlackoutProbe />, store, transport));
		await waitFor(() =>
			expect(screen.getByText("Blackout false")).toBeInTheDocument(),
		);
		const readyRenders = renders.mock.calls.length;

		act(() =>
			transport.emit({
				type: "event",
				sequence: 11,
				correlationId: null,
				change: {
					projection: outputProjection({
						revision: 2,
						grandMaster: 0.4,
					}),
				},
			}),
		);
		expect(renders).toHaveBeenCalledTimes(readyRenders);

		act(() =>
			transport.emit({
				type: "event",
				sequence: 12,
				correlationId: null,
				change: {
					projection: outputProjection({
						revision: 3,
						grandMaster: 0.4,
						blackout: true,
					}),
				},
			}),
		);
		expect(screen.getByText("Blackout true")).toBeInTheDocument();
		expect(renders).toHaveBeenCalledTimes(readyRenders + 1);
	});
});
