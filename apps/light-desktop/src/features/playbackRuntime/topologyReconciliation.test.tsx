import { StrictMode } from "react";
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
	PlaybackRuntimeViewProvider,
	usePlaybackProjectionMap,
} from "./PlaybackRuntimeView";
import { PlaybackRuntimeStore } from "./store";
import {
	CUE_LIST_ID,
	DESK_ID,
	SHOW_ID,
	playbackSnapshot,
} from "./testFixtures";
import {
	playbackTargetMatches,
	usePlaybackTopologyRuntimeReconciliation,
} from "./topologyReconciliation";
import type { PlaybackSnapshot } from "./contracts";

afterEach(cleanup);

const NEXT = "55555555-5555-4555-8555-555555555555";
function Probe({ target }: { target: string }) {
	const projection = usePlaybackProjectionMap([21]).get(21);
	const matches = playbackTargetMatches(
		{ type: "cue_list", cue_list_id: target },
		projection,
	);
	const pending = usePlaybackTopologyRuntimeReconciliation(
		`21:${target}`,
		projection !== undefined,
		matches,
	);
	return (
		<span>
			{pending || !projection
				? "Loading"
				: matches
					? `GO ${projection.target === "cue_list" ? projection.cue_list_id : ""}`
					: "Authority error"}
		</span>
	);
}
function harness() {
	const store = new PlaybackRuntimeStore();
	let resolve: (value: PlaybackSnapshot) => void = () => {};
	const load = vi.fn(async (identities) => {
		if (load.mock.calls.length === 1) {
			const snapshot = playbackSnapshot(identities);
			snapshot.projections = snapshot.projections.map((projection) =>
				projection.target === "cue_list"
					? { ...projection, runtime: null }
					: projection,
			);
			return snapshot;
		}
		return new Promise<PlaybackSnapshot>((done) => {
			resolve = done;
		});
	});
	const wrap = (target: string) => (
		<PlaybackRuntimeViewProvider
			showId={SHOW_ID}
			deskId={DESK_ID}
			authorityKey="desk-a"
			store={store}
			transport={null}
			loadSnapshot={load}
		>
			<Probe target={target} />
		</PlaybackRuntimeViewProvider>
	);
	return {
		store,
		load,
		wrap,
		resolve: (target: string) => {
			const snapshot = playbackSnapshot([
				{ kind: "playback", playback_number: 21 },
			]);
			snapshot.projections = snapshot.projections.map((projection) =>
				projection.target === "cue_list"
					? { ...projection, cue_list_id: target }
					: projection,
			);
			resolve(snapshot);
		},
	};
}
describe("visible playback reassignment reconciliation", () => {
	it("refreshes unchanged physical identity once, withholding GO until the new Cuelist matches", async () => {
		const h = harness();
		const view = render(h.wrap(CUE_LIST_ID));
		await screen.findByText(`GO ${CUE_LIST_ID}`);
		view.rerender(h.wrap(NEXT));
		expect(screen.getByText("Loading")).toBeVisible();
		await waitFor(() => expect(h.load).toHaveBeenCalledTimes(2));
		await act(async () => h.resolve(NEXT));
		await screen.findByText(`GO ${NEXT}`);
		expect(h.load).toHaveBeenCalledTimes(2);
		expect(h.load.mock.calls[1][0]).toEqual([
			{ kind: "playback", playback_number: 21 },
		]);
	});
	it("completes reconciliation through StrictMode effect replay", async () => {
		const h = harness();
		const view = render(<StrictMode>{h.wrap(CUE_LIST_ID)}</StrictMode>);
		await screen.findByText(`GO ${CUE_LIST_ID}`);
		view.rerender(<StrictMode>{h.wrap(NEXT)}</StrictMode>);
		await waitFor(() => expect(h.load).toHaveBeenCalledTimes(2));
		await act(async () => h.resolve(NEXT));
		await screen.findByText(`GO ${NEXT}`);
	});
	it("keeps a fresh contradictory authority blocked and does not endlessly refresh", async () => {
		const h = harness();
		const view = render(h.wrap(CUE_LIST_ID));
		await screen.findByText(`GO ${CUE_LIST_ID}`);
		view.rerender(h.wrap(NEXT));
		await waitFor(() => expect(h.load).toHaveBeenCalledTimes(2));
		await act(async () => h.resolve(CUE_LIST_ID));
		await screen.findByText("Authority error");
		expect(h.load).toHaveBeenCalledTimes(2);
	});
	it("serializes a second reassignment behind an in-flight repair without applying its stale target", async () => {
		const h = harness();
		const view = render(h.wrap(CUE_LIST_ID));
		await screen.findByText(`GO ${CUE_LIST_ID}`);
		view.rerender(h.wrap(NEXT));
		await waitFor(() => expect(h.load).toHaveBeenCalledTimes(2));
		const third = "66666666-6666-4666-8666-666666666666";
		view.rerender(h.wrap(third));
		await act(async () => h.resolve(NEXT));
		expect(screen.queryByText(`GO ${NEXT}`)).not.toBeInTheDocument();
		await waitFor(() => expect(h.load).toHaveBeenCalledTimes(3));
		await act(async () => h.resolve(third));
		await screen.findByText(`GO ${third}`);
	});
});
