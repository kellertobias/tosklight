import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
	SpeedGroupRuntimeProvider,
	useSpeedGroupRuntimeActions,
	useSpeedGroupRuntimeView,
} from "./SpeedGroupRuntimeView";
import { SpeedGroupRuntimeStore } from "./store";
import {
	AUTHORITY_ID,
	DESK_ID,
	deferred,
	FakeSpeedGroupRuntimeTransport,
	OTHER_DESK_ID,
	settleSpeedGroupSession,
	speedGroup,
	speedSnapshot,
} from "./testFixtures";

afterEach(cleanup);

function DisabledProbe() {
	const view = useSpeedGroupRuntimeView(false);
	const actions = useSpeedGroupRuntimeActions(false);
	return <span>{`${view.status}:${String(actions)}`}</span>;
}

function ViewProbe() {
	const view = useSpeedGroupRuntimeView();
	return (
		<span>
			{view.ready
				? `${view.projection?.groups[0]?.manualBpm}:${view.pending}`
				: "Speed loading"}
		</span>
	);
}

function provider(
	child: ReactNode,
	store: SpeedGroupRuntimeStore,
	transport: FakeSpeedGroupRuntimeTransport,
	authorityKey = "session-a",
	deskId = DESK_ID,
) {
	return (
		<SpeedGroupRuntimeProvider
			deskId={deskId}
			authorityKey={authorityKey}
			store={store}
			transport={transport}
		>
			{child}
		</SpeedGroupRuntimeProvider>
	);
}

describe("SpeedGroupRuntimeProvider", () => {
	it("mounts dormant until the first enabled Speed Group view", async () => {
		const store = new SpeedGroupRuntimeStore();
		const transport = new FakeSpeedGroupRuntimeTransport();
		const subscribe = vi.spyOn(store, "subscribe");
		const rendered = render(provider(<DisabledProbe />, store, transport));

		expect(screen.getByText("idle:null")).toBeInTheDocument();
		await settleSpeedGroupSession();
		expect(subscribe).not.toHaveBeenCalled();
		expect(transport.loadSnapshot).not.toHaveBeenCalled();
		expect(transport.subscribe).not.toHaveBeenCalled();
		expect(transport.applyAction).not.toHaveBeenCalled();

		rendered.rerender(provider(<ViewProbe />, store, transport));
		await waitFor(() =>
			expect(screen.getByText("120:false")).toBeInTheDocument(),
		);
		expect(transport.loadSnapshot).toHaveBeenCalledOnce();
		expect(transport.subscribe).toHaveBeenCalledWith(
			{ deskId: DESK_ID },
			10,
			expect.any(Object),
		);
	});

	it("repairs a cursor gap from one narrow snapshot and stream repair", async () => {
		const store = new SpeedGroupRuntimeStore();
		const transport = new FakeSpeedGroupRuntimeTransport();
		render(provider(<ViewProbe />, store, transport));
		await waitFor(() =>
			expect(screen.getByText("120:false")).toBeInTheDocument(),
		);
		transport.loadSnapshot.mockResolvedValueOnce(
			speedSnapshot({
				cursor: 20,
				revision: 3,
				groups: [
					speedGroup("A", { manualBpm: 140 }),
					speedGroup("B"),
					speedGroup("C"),
					speedGroup("D"),
					speedGroup("E"),
				],
			}),
		);

		act(() =>
			transport.emit({
				type: "gap",
				afterSequence: 10,
				oldestAvailable: 15,
				latestSequence: 19,
			}),
		);
		await waitFor(() =>
			expect(screen.getByText("140:false")).toBeInTheDocument(),
		);
		expect(transport.loadSnapshot).toHaveBeenCalledTimes(2);
		expect(transport.subscriptions[0]?.repair).toHaveBeenCalledWith(20);
	});

	it("drops old hydration across desk and server-session replacement", async () => {
		const store = new SpeedGroupRuntimeStore();
		const first = new FakeSpeedGroupRuntimeTransport();
		const second = new FakeSpeedGroupRuntimeTransport();
		const pending = deferred<ReturnType<typeof speedSnapshot>>();
		first.loadSnapshot.mockReturnValueOnce(pending.promise);
		const rendered = render(provider(<ViewProbe />, store, first));
		await waitFor(() => expect(first.loadSnapshot).toHaveBeenCalledOnce());

		rendered.rerender(
			provider(<ViewProbe />, store, second, "session-b", OTHER_DESK_ID),
		);
		pending.resolve(speedSnapshot({ revision: 99 }));
		await act(settleSpeedGroupSession);
		await waitFor(() =>
			expect(screen.getByText("120:false")).toBeInTheDocument(),
		);
		expect(first.subscribe).not.toHaveBeenCalled();
		expect(second.loadSnapshot).toHaveBeenCalledWith({ deskId: OTHER_DESK_ID });
		expect(store.getSnapshot()).toMatchObject({
			deskId: OTHER_DESK_ID,
			authorityKey: "session-b",
			authorityId: AUTHORITY_ID,
			authorityRevision: 1,
		});
	});

	it("updates its external-store consumer without rerendering a sibling", async () => {
		const store = new SpeedGroupRuntimeStore();
		const transport = new FakeSpeedGroupRuntimeTransport();
		const siblingRenders = vi.fn();
		function Sibling() {
			siblingRenders();
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
			expect(screen.getByText("120:false")).toBeInTheDocument(),
		);
		const renders = siblingRenders.mock.calls.length;

		act(() =>
			transport.emit({
				type: "event",
				sequence: 11,
				correlationId: null,
				change: {
					authorityId: AUTHORITY_ID,
					revision: 2,
					appliedAtMillis: 200,
					groups: [speedGroup("A", { manualBpm: 128 })],
				},
			}),
		);
		expect(screen.getByText("128:false")).toBeInTheDocument();
		expect(siblingRenders).toHaveBeenCalledTimes(renders);
	});
});
