import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from "vitest";
import type { PlaybackRuntimeActionApply } from "../playbackRuntime/actionWriter";
import type {
	PlaybackIdentity,
	PlaybackProjection,
	PlaybackSnapshot,
} from "../playbackRuntime/contracts";
import { PlaybackRuntimeViewProvider } from "../playbackRuntime/PlaybackRuntimeView";
import { PlaybackRuntimeStore } from "../playbackRuntime/store";
import {
	cueProjection,
	DESK_ID,
	playbackSnapshot,
	SHOW_ID,
} from "../playbackRuntime/testFixtures";
import type { ShowObject } from "../showObjects/contracts";

const mocks = vi.hoisted(() => ({
	pagesView: {
		ready: true,
		error: null as Error | null,
		pages: [] as ShowObject<"playback_page">[],
	},
}));

vi.mock("../playbackTopology/PlaybackTopologyView", () => ({
	usePlaybackPagesView: () => mocks.pagesView,
}));

const { DemoPlaybackControls } = await import("./DemoPlaybackControls");

type SnapshotLoader = (
	identities: PlaybackIdentity[],
) => Promise<PlaybackSnapshot>;
type SnapshotLoaderMock = Mock<SnapshotLoader>;
type ApplyActionMock = Mock<PlaybackRuntimeActionApply>;

const DEMO_PAGE: ShowObject<"playback_page"> = {
	kind: "playback_page",
	id: "demo-page-1",
	revision: 1,
	updated_at: "2026-07-21T04:00:00Z",
	body: {
		number: 1,
		name: "Page 1",
		slots: {
			"1": 11,
			"2": 12,
			"3": 13,
			"4": 14,
			"21": 21,
			"22": 22,
			"23": 23,
			"24": 24,
		},
	},
};

function renderControls(
	loadSnapshot: SnapshotLoaderMock,
	applyAction: ApplyActionMock | null = unexpectedAction(),
) {
	return render(
		<PlaybackRuntimeViewProvider
			showId={SHOW_ID}
			deskId={DESK_ID}
			authorityKey="demo-authority"
			store={new PlaybackRuntimeStore()}
			transport={null}
			loadSnapshot={loadSnapshot}
			applyAction={applyAction}
		>
			<DemoPlaybackControls />
		</PlaybackRuntimeViewProvider>,
	);
}

function unexpectedAction(): ApplyActionMock {
	return vi.fn(async () => {
		throw new Error("The loading demo must not send an action");
	});
}

function allPlaybackButtons() {
	return screen.getAllByRole("button", { name: /^Playback \d+ button \d+$/ });
}

function allFaders() {
	return [1, 2, 3, 4].map((slot) =>
		screen.getByLabelText(`Playback ${slot} fader`),
	);
}

function expectEveryControlDisabled() {
	for (const control of [...allPlaybackButtons(), ...allFaders()])
		expect(control).toBeDisabled();
}

function deferredSnapshot() {
	let resolve!: (snapshot: PlaybackSnapshot) => void;
	const promise = new Promise<PlaybackSnapshot>((settle) => {
		resolve = settle;
	});
	return { promise, resolve };
}

function projectionsFor(identities: PlaybackIdentity[]) {
	return identities.map((identity) => {
		if (identity.kind !== "playback")
			throw new Error("The demo requests only Playback identities");
		return cueProjection(identity.playback_number);
	});
}

beforeEach(() => {
	mocks.pagesView = { ready: true, error: null, pages: [DEMO_PAGE] };
	if (!HTMLElement.prototype.setPointerCapture)
		HTMLElement.prototype.setPointerCapture = () => undefined;
});

afterEach(cleanup);

describe("DemoPlaybackControls runtime readiness", () => {
	it("refuses every control until all mapped runtime projections hydrate", async () => {
		const deferred = deferredSnapshot();
		const loadSnapshot: SnapshotLoaderMock = vi.fn((identities) =>
			identities.length
				? deferred.promise
				: Promise.resolve(playbackSnapshot(identities)),
		);
		const applyAction = unexpectedAction();
		renderControls(loadSnapshot, applyAction);

		await waitFor(() => expect(loadSnapshot).toHaveBeenCalledTimes(2));
		expect(screen.getByRole("status")).toHaveTextContent(
			"Loading Playback controls…",
		);
		expectEveryControlDisabled();
		expect(screen.queryByText("0%")).not.toBeInTheDocument();

		fireEvent.pointerDown(screen.getByRole("button", { name: "Playback 21 button 1" }));
		fireEvent.input(screen.getByLabelText("Playback 1 fader"), {
			target: { value: "0.5" },
		});
		expect(applyAction).not.toHaveBeenCalled();

		const identities = loadSnapshot.mock.calls[1][0];
		act(() => deferred.resolve(playbackSnapshot(identities)));
		await waitFor(() => expect(screen.queryByRole("status")).not.toBeInTheDocument());
		for (const control of [...allPlaybackButtons(), ...allFaders()])
			expect(control).toBeEnabled();
		expect(screen.getAllByText("100%")).toHaveLength(4);
	});

	it("does not treat an omitted requested projection as authority", async () => {
		const applyAction = unexpectedAction();
		const loadSnapshot: SnapshotLoaderMock = vi.fn(async (identities) =>
			playbackSnapshot(identities, 10, projectionsFor(identities).slice(0, -1)),
		);
		renderControls(loadSnapshot, applyAction);

		await waitFor(() => expect(loadSnapshot).toHaveBeenCalledOnce());
		await waitFor(() => expect(screen.getByRole("status")).toBeInTheDocument());
		expectEveryControlDisabled();
		fireEvent.pointerDown(screen.getByRole("button", { name: "Playback 24 button 1" }));
		expect(applyAction).not.toHaveBeenCalled();
	});

	it("accepts an explicit missing projection as hydrated authority", async () => {
		const loadSnapshot: SnapshotLoaderMock = vi.fn(async (identities) => {
			const projections = projectionsFor(identities).map((projection) =>
				projection.playback_number === 24 ? missingProjection(24) : projection,
			);
			return playbackSnapshot(identities, 10, projections);
		});
		renderControls(loadSnapshot);

		await waitFor(() =>
			expect(
				screen.getByRole("button", { name: "Playback 24 button 1" }),
			).toBeEnabled(),
		);
		expect(screen.queryByRole("status")).not.toBeInTheDocument();
	});

	it("renders an accessible error and refuses actions after hydration fails", async () => {
		const applyAction = unexpectedAction();
		const loadSnapshot: SnapshotLoaderMock = vi.fn(async (identities) => {
			if (!identities.length) return playbackSnapshot(identities);
			throw new Error("Demo Playback runtime is offline");
		});
		renderControls(loadSnapshot, applyAction);

		await waitFor(() =>
			expect(screen.getByRole("alert")).toHaveTextContent(
				"Demo Playback runtime is offline",
			),
		);
		expectEveryControlDisabled();
		expect(applyAction).not.toHaveBeenCalled();
	});

	it("reports an absent runtime writer instead of enabling read-only controls", async () => {
		const loadSnapshot: SnapshotLoaderMock = vi.fn(async (identities) =>
			playbackSnapshot(identities),
		);
		renderControls(loadSnapshot, null);

		await waitFor(() =>
			expect(screen.getByRole("alert")).toHaveTextContent(
				"Playback controls are unavailable.",
			),
		);
		expectEveryControlDisabled();
	});
});

function missingProjection(playbackNumber: number): PlaybackProjection {
	return {
		scope: { show_id: SHOW_ID, show_revision: 4 },
		requested: { kind: "playback", playback_number: playbackNumber },
		playback_number: playbackNumber,
		target: "missing",
	};
}
