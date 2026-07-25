import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { useOutputRuntimeView } from "../features/outputRuntime/OutputRuntimeView";
import { OutputRuntimeStore } from "../features/outputRuntime/store";
import {
	deferred,
	DESK_ID,
	FakeOutputRuntimeTransport,
	OTHER_DESK_ID,
	OTHER_SHOW_ID,
	outputSnapshot,
	settleOutputSession,
	SHOW_ID,
} from "../features/outputRuntime/testFixtures";
import type { useServerState } from "../features/server/useServerState";
import { ServerOutputRuntimeBoundary } from "./ServerProgrammingProviders";
import type { useServerFeatureBoundaries } from "./useServerFeatureBoundaries";

afterEach(cleanup);

function Probe({ enabled = true }: { enabled?: boolean }) {
	const view = useOutputRuntimeView(enabled);
	return (
		<span>
			{view.ready
				? `${view.projection?.showId}:${view.projection?.grandMaster}`
				: "Output loading"}
		</span>
	);
}

function state(store: OutputRuntimeStore, showId = SHOW_ID, deskId = DESK_ID) {
	return {
		bootstrap: { active_show: { id: showId } },
		session: { desk: { id: deskId } },
		outputRuntimeStore: store,
	} as unknown as ReturnType<typeof useServerState>;
}

function boundaries(
	transport: FakeOutputRuntimeTransport,
	authorityKey = "server-session-a",
) {
	return {
		outputRuntimeTransport: transport,
		outputRuntimeAuthorityKey: authorityKey,
		reportOutputRuntimeSessionError: () => undefined,
		reportOutputRuntimeMutationError: () => undefined,
	} as unknown as ReturnType<typeof useServerFeatureBoundaries>;
}

describe("ServerOutputRuntimeBoundary", () => {
	it("constructs dormant and activates only the exact mounted Show and desk", async () => {
		const store = new OutputRuntimeStore();
		const transport = new FakeOutputRuntimeTransport();
		const rendered = render(
			<ServerOutputRuntimeBoundary
				state={state(store)}
				boundaries={boundaries(transport)}
			>
				<Probe enabled={false} />
			</ServerOutputRuntimeBoundary>,
		);
		await settleOutputSession();

		expect(transport.loadSnapshot).not.toHaveBeenCalled();
		expect(transport.subscribe).not.toHaveBeenCalled();
		expect(transport.applyAction).not.toHaveBeenCalled();

		rendered.rerender(
			<ServerOutputRuntimeBoundary
				state={state(store)}
				boundaries={boundaries(transport)}
			>
				<Probe />
			</ServerOutputRuntimeBoundary>,
		);
		await waitFor(() =>
			expect(screen.getByText(`${SHOW_ID}:1`)).toBeInTheDocument(),
		);
		expect(transport.loadSnapshot).toHaveBeenCalledWith({
			showId: SHOW_ID,
			deskId: DESK_ID,
		});
		expect(transport.subscribe).toHaveBeenCalledWith(
			{ showId: SHOW_ID, deskId: DESK_ID },
			10,
			expect.any(Object),
		);
	});

	it("drops an old snapshot across Show, desk, and server-session replacement", async () => {
		const store = new OutputRuntimeStore();
		const first = new FakeOutputRuntimeTransport();
		const second = new FakeOutputRuntimeTransport();
		const pending = deferred<ReturnType<typeof outputSnapshot>>();
		first.loadSnapshot.mockReturnValueOnce(pending.promise);
		const rendered = render(
			<ServerOutputRuntimeBoundary
				state={state(store)}
				boundaries={boundaries(first)}
			>
				<Probe />
			</ServerOutputRuntimeBoundary>,
		);
		await waitFor(() => expect(first.loadSnapshot).toHaveBeenCalledOnce());

		rendered.rerender(
			<ServerOutputRuntimeBoundary
				state={state(store, OTHER_SHOW_ID, OTHER_DESK_ID)}
				boundaries={boundaries(second, "server-session-b")}
			>
				<Probe />
			</ServerOutputRuntimeBoundary>,
		);
		pending.resolve(outputSnapshot({ revision: 99 }));
		await act(settleOutputSession);
		await waitFor(() =>
			expect(screen.getByText(`${OTHER_SHOW_ID}:1`)).toBeInTheDocument(),
		);

		expect(first.subscribe).not.toHaveBeenCalled();
		expect(second.loadSnapshot).toHaveBeenCalledWith({
			showId: OTHER_SHOW_ID,
			deskId: OTHER_DESK_ID,
		});
		expect(store.getSnapshot()).toMatchObject({
			showId: OTHER_SHOW_ID,
			deskId: OTHER_DESK_ID,
			authorityKey: "server-session-b",
		});
	});
});
