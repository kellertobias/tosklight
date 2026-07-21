import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import type { useServerState } from "../features/server/useServerState";
import { useSpeedGroupRuntimeView } from "../features/speedGroupRuntime/SpeedGroupRuntimeView";
import { SpeedGroupRuntimeStore } from "../features/speedGroupRuntime/store";
import {
	DESK_ID,
	FakeSpeedGroupRuntimeTransport,
	settleSpeedGroupSession,
} from "../features/speedGroupRuntime/testFixtures";
import { ServerSpeedGroupRuntimeBoundary } from "./ServerProgrammingProviders";
import type { useServerFeatureBoundaries } from "./useServerFeatureBoundaries";

afterEach(cleanup);

function Probe({ enabled = true }: { enabled?: boolean }) {
	const view = useSpeedGroupRuntimeView(enabled);
	return (
		<span>
			{view.ready ? view.projection?.groups[0]?.manualBpm : "Speed loading"}
		</span>
	);
}

function state(store: SpeedGroupRuntimeStore) {
	return {
		session: { desk: { id: DESK_ID } },
		speedGroupRuntimeStore: store,
	} as unknown as ReturnType<typeof useServerState>;
}

function boundaries(transport: FakeSpeedGroupRuntimeTransport) {
	return {
		speedGroupRuntimeTransport: transport,
		speedGroupRuntimeAuthorityKey: "server-session-a",
		reportSpeedGroupSessionError: () => undefined,
		reportSpeedGroupMutationError: () => undefined,
	} as unknown as ReturnType<typeof useServerFeatureBoundaries>;
}

describe("ServerSpeedGroupRuntimeBoundary", () => {
	it("mounts the retained provider dormant, then scopes it to the desk", async () => {
		const store = new SpeedGroupRuntimeStore();
		const transport = new FakeSpeedGroupRuntimeTransport();
		const rendered = render(
			<ServerSpeedGroupRuntimeBoundary
				state={state(store)}
				boundaries={boundaries(transport)}
			>
				<Probe enabled={false} />
			</ServerSpeedGroupRuntimeBoundary>,
		);
		await settleSpeedGroupSession();
		expect(transport.loadSnapshot).not.toHaveBeenCalled();
		expect(transport.subscribe).not.toHaveBeenCalled();

		rendered.rerender(
			<ServerSpeedGroupRuntimeBoundary
				state={state(store)}
				boundaries={boundaries(transport)}
			>
				<Probe />
			</ServerSpeedGroupRuntimeBoundary>,
		);
		await waitFor(() => expect(screen.getByText("120")).toBeInTheDocument());
		expect(transport.loadSnapshot).toHaveBeenCalledWith({ deskId: DESK_ID });
		expect(transport.subscribe).toHaveBeenCalledWith(
			{ deskId: DESK_ID },
			10,
			expect.any(Object),
		);
	});
});
