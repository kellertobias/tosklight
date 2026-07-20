import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { useCallback } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
	ProgrammerPreloadPlaybackQueueViewProvider,
	useProgrammerPreloadPlaybackQueueSelector,
} from "./ProgrammerPreloadPlaybackQueueView";
import { ProgrammerPreloadPlaybackQueueStore } from "./store";
import {
	AUTHORITY_A,
	FakeProgrammerPreloadPlaybackQueueTransport,
	queuedPlayback,
	queueProjection,
	queueSnapshot,
	SHOW_ID,
	USER_ID,
} from "./testFixtures";

function StaticProbe() {
	return <span>Dormant child</span>;
}

function FirstPlaybackProbe({ onRender }: { onRender: () => void }) {
	onRender();
	const selectFirst = useCallback(
		(state: ReturnType<ProgrammerPreloadPlaybackQueueStore["getSnapshot"]>) =>
			state.projection?.actions[0]?.playbackNumber ?? null,
		[],
	);
	const playbackNumber = useProgrammerPreloadPlaybackQueueSelector(selectFirst);
	return <span>{playbackNumber ?? "Loading"}</span>;
}

afterEach(cleanup);

describe("ProgrammerPreloadPlaybackQueueViewProvider", () => {
	it("stays dormant when no queue view is mounted", async () => {
		const store = new ProgrammerPreloadPlaybackQueueStore();
		const transport = new FakeProgrammerPreloadPlaybackQueueTransport();
		const loadSnapshot = vi.fn(async () => queueSnapshot());

		render(
			<ProgrammerPreloadPlaybackQueueViewProvider
				showId={SHOW_ID}
				userId={USER_ID}
				authorityKey={AUTHORITY_A}
				store={store}
				transport={transport}
				loadSnapshot={loadSnapshot}
			>
				<StaticProbe />
			</ProgrammerPreloadPlaybackQueueViewProvider>,
		);
		await act(async () => Promise.resolve());

		expect(screen.getByText("Dormant child")).toBeInTheDocument();
		expect(loadSnapshot).not.toHaveBeenCalled();
		expect(transport.subscriptions).toHaveLength(0);
	});

	it("does not rerender a selector when only later queue entries change", async () => {
		const store = new ProgrammerPreloadPlaybackQueueStore();
		const transport = new FakeProgrammerPreloadPlaybackQueueTransport();
		const onRender = vi.fn();
		render(
			<ProgrammerPreloadPlaybackQueueViewProvider
				showId={SHOW_ID}
				userId={USER_ID}
				authorityKey={AUTHORITY_A}
				store={store}
				transport={transport}
				loadSnapshot={async () => queueSnapshot()}
			>
				<FirstPlaybackProbe onRender={onRender} />
			</ProgrammerPreloadPlaybackQueueViewProvider>,
		);
		await waitFor(() => expect(screen.getByText("7")).toBeInTheDocument());
		const rendersBefore = onRender.mock.calls.length;

		act(() =>
			transport.emit({
				type: "event",
				sequence: 11,
				correlationId: "append",
				projection: queueProjection({
					revision: 3,
					actions: [
						queuedPlayback(),
						queuedPlayback({ playbackNumber: 9, action: "toggle" }),
					],
				}),
			}),
		);

		expect(screen.getByText("7")).toBeInTheDocument();
		expect(onRender).toHaveBeenCalledTimes(rendersBefore);
	});
});
