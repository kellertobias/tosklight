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
import type { ProgrammerPriorityActionOutcome } from "./contracts";
import {
	ProgrammerPriorityProvider,
	useProgrammerPriorityActions,
	useProgrammerPriorityView,
} from "./ProgrammerPriorityView";
import { ProgrammerPriorityStore } from "./store";
import {
	changedOutcome,
	deferred,
	FakeProgrammerPriorityTransport,
	priorityProjection,
	settlePrioritySession,
	USER_ID,
} from "./testFixtures";

afterEach(cleanup);

function DisabledProbe() {
	const view = useProgrammerPriorityView(false);
	const actions = useProgrammerPriorityActions(false);
	return <span>{`${view.status}:${String(actions)}`}</span>;
}

function ViewProbe() {
	const view = useProgrammerPriorityView();
	return (
		<span>
			{view.ready ? `${view.projection?.priority}:${view.pending}` : "Loading"}
		</span>
	);
}

function ActionProbe({
	onOutcome,
}: {
	onOutcome: (outcome: ProgrammerPriorityActionOutcome | null) => void;
}) {
	const actions = useProgrammerPriorityActions();
	return (
		<button
			type="button"
			onClick={() =>
				void actions
					?.setPriority({ priority: 9, requestId: "late-response" })
					.then(onOutcome)
			}
		>
			Set priority
		</button>
	);
}

function provider(
	child: ReactNode,
	store: ProgrammerPriorityStore,
	transport: FakeProgrammerPriorityTransport,
	authorityKey = "session-a",
) {
	return (
		<ProgrammerPriorityProvider
			userId={USER_ID}
			authorityKey={authorityKey}
			store={store}
			transport={transport}
		>
			{child}
		</ProgrammerPriorityProvider>
	);
}

describe("ProgrammerPriorityProvider", () => {
	it("mounts completely dormant until an enabled view or action owner appears", async () => {
		const store = new ProgrammerPriorityStore();
		const transport = new FakeProgrammerPriorityTransport();
		const subscribe = vi.spyOn(store, "subscribe");
		const rendered = render(provider(<DisabledProbe />, store, transport));

		expect(screen.getByText("idle:null")).toBeInTheDocument();
		await settlePrioritySession();
		expect(subscribe).not.toHaveBeenCalled();
		expect(transport.loadSnapshot).not.toHaveBeenCalled();
		expect(transport.subscribe).not.toHaveBeenCalled();
		expect(transport.applyAction).not.toHaveBeenCalled();

		rendered.rerender(provider(<ViewProbe />, store, transport));
		await waitFor(() =>
			expect(screen.getByText("0:false")).toBeInTheDocument(),
		);
		expect(transport.loadSnapshot).toHaveBeenCalledOnce();
		expect(transport.subscribe).toHaveBeenCalledOnce();
	});

	it("drops a late mutation response after session authority replacement", async () => {
		const store = new ProgrammerPriorityStore();
		const transport = new FakeProgrammerPriorityTransport();
		const response = deferred<ProgrammerPriorityActionOutcome>();
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
			expect(screen.getByText("0:false")).toBeInTheDocument(),
		);

		fireEvent.click(screen.getByRole("button", { name: "Set priority" }));
		await waitFor(() => expect(screen.getByText("9:true")).toBeInTheDocument());
		const request = transport.applyAction.mock.calls[0]?.[1];
		if (!request) throw new Error("missing priority request");

		rendered.rerender(provider(child, store, transport, "session-b"));
		await waitFor(() =>
			expect(screen.getByText("0:false")).toBeInTheDocument(),
		);
		response.resolve(
			changedOutcome(
				request.requestId,
				priorityProjection({ revision: 2, priority: 9 }),
			),
		);
		await act(async () => response.promise);

		expect(onOutcome).toHaveBeenCalledWith(null);
		expect(store.getSnapshot()).toMatchObject({
			authorityKey: "session-b",
			authorityRevision: 1,
			projection: { priority: 0 },
		});
	});

	it("does not rerender unrelated children for scoped priority events", async () => {
		const store = new ProgrammerPriorityStore();
		const transport = new FakeProgrammerPriorityTransport();
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
			expect(screen.getByText("0:false")).toBeInTheDocument(),
		);
		const renders = onSiblingRender.mock.calls.length;

		act(() =>
			transport.emit({
				type: "event",
				sequence: 11,
				correlationId: null,
				change: {
					type: "upsert",
					projection: priorityProjection({ revision: 2, priority: 6 }),
				},
			}),
		);

		expect(screen.getByText("6:false")).toBeInTheDocument();
		expect(onSiblingRender).toHaveBeenCalledTimes(renders);
	});
});
