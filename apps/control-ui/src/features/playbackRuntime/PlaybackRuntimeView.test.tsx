import { act, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import {
	PlaybackRuntimeViewProvider,
	usePlaybackRuntime,
} from "./PlaybackRuntimeView";
import { PlaybackRuntimeStore } from "./store";
import {
	cueProjection,
	DESK_ID,
	playbackSnapshot,
	SHOW_ID,
} from "./testFixtures";
import type {
	PlaybackEventObserver,
	PlaybackEventScope,
	PlaybackEventTransport,
} from "./transport";

class FakeTransport implements PlaybackEventTransport {
	readonly subscriptions: Array<{
		scope: PlaybackEventScope;
		observer: PlaybackEventObserver;
		close: ReturnType<typeof vi.fn>;
	}> = [];

	subscribe(
		_deskId: string,
		scope: PlaybackEventScope,
		_after: number | null,
		observer: PlaybackEventObserver,
	) {
		const close = vi.fn();
		this.subscriptions.push({ scope, observer, close });
		return { close, repair: vi.fn() };
	}
}

function RuntimeProbe({
	visible,
	onRender,
}: {
	visible: boolean;
	onRender: () => void;
}) {
	onRender();
	const runtime = usePlaybackRuntime(visible ? 1 : null);
	return <span>{runtime ? `Cue ${runtime.cue_index + 1}` : "Hidden"}</span>;
}

describe("PlaybackRuntimeViewProvider", () => {
	it("does not fetch or subscribe a hidden view and isolates irrelevant events", async () => {
		const store = new PlaybackRuntimeStore();
		const transport = new FakeTransport();
		const loadSnapshot = vi.fn(async (identities) =>
			playbackSnapshot(identities),
		);
		const onRender = vi.fn();
		const view = (visible: boolean) => (
			<PlaybackRuntimeViewProvider
				showId={SHOW_ID}
				deskId={DESK_ID}
				store={store}
				transport={transport}
				loadSnapshot={loadSnapshot}
			>
				<RuntimeProbe visible={visible} onRender={onRender} />
			</PlaybackRuntimeViewProvider>
		);
		const rendered = render(view(false));
		expect(screen.getByText("Hidden")).toBeInTheDocument();
		expect(loadSnapshot).not.toHaveBeenCalled();
		expect(transport.subscriptions).toHaveLength(0);

		rendered.rerender(view(true));
		await waitFor(() => expect(transport.subscriptions).toHaveLength(1));
		expect(loadSnapshot).toHaveBeenCalledWith([
			{ kind: "playback", playback_number: 1 },
		]);
		expect(transport.subscriptions[0].scope).toEqual({
			identities: [{ kind: "playback", playback_number: 1 }],
			desk: false,
		});
		await waitFor(() => expect(screen.getByText("Cue 1")).toBeInTheDocument());
		const beforeIrrelevant = onRender.mock.calls.length;
		act(() =>
			transport.subscriptions[0].observer.message({
				type: "event",
				sequence: 11,
				payload: { type: "runtime", projection: cueProjection(2, 3) },
			}),
		);
		expect(onRender).toHaveBeenCalledTimes(beforeIrrelevant);

		act(() =>
			transport.subscriptions[0].observer.message({
				type: "event",
				sequence: 12,
				payload: { type: "runtime", projection: cueProjection(1, 2) },
			}),
		);
		await waitFor(() => expect(screen.getByText("Cue 3")).toBeInTheDocument());

		rendered.rerender(view(false));
		await waitFor(() =>
			expect(transport.subscriptions[0].close).toHaveBeenCalledOnce(),
		);
		expect(screen.getByText("Hidden")).toBeInTheDocument();
	});

	it("hydrates a visible runtime over REST without opening a socket", async () => {
		const store = new PlaybackRuntimeStore();
		const loadSnapshot = vi.fn(async (identities) =>
			playbackSnapshot(identities),
		);
		render(
			<PlaybackRuntimeViewProvider
				showId={SHOW_ID}
				deskId={DESK_ID}
				store={store}
				transport={null}
				loadSnapshot={loadSnapshot}
			>
				<RuntimeProbe visible onRender={() => undefined} />
			</PlaybackRuntimeViewProvider>,
		);
		await waitFor(() => expect(screen.getByText("Cue 1")).toBeInTheDocument());
		expect(loadSnapshot).toHaveBeenCalledOnce();
	});
});
