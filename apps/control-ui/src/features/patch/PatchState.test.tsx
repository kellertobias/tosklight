import { render } from "@testing-library/react";
import { act } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PatchedFixture } from "../../api/types";
import type {
	PatchFixtureProjection,
	PatchProfileRevision,
} from "./contracts";
import {
	usePatchedFixtures,
	usePatchFixture,
	usePatchFixturesById,
	usePatchFixturesForSelection,
	usePatchStatus,
} from "./PatchState";
import { PatchStore } from "./store";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";

const { storeRef } = vi.hoisted(() => ({
	storeRef: { current: null as PatchStore | null },
}));

vi.mock("./PatchContext", () => ({
	usePatchStoreOrNull: () => storeRef.current,
}));

const PROFILE_ID = "40000000-0000-0000-0000-000000000001";
const MODE_ID = "50000000-0000-0000-0000-000000000001";

function profileProjection(): PatchProfileRevision {
	return {
		profileId: PROFILE_ID,
		profileRevision: 3,
		contentDigest: "digest",
		manufacturer: "Acme",
		name: "Atomic Wash",
		fixtureType: "wash",
		patchPolicy: "dmx",
		referencedModes: [
			{
				modeId: MODE_ID,
				name: "16 channel",
				splits: [{ split: 1, footprint: 16 }],
			},
		],
	};
}

function fixtureProjection(
	fixtureId: string,
	fixtureNumber: number,
	heads: string[] = [],
): PatchFixtureProjection {
	return {
		fixtureId,
		fixtureRevision: 1,
		fixtureNumber,
		virtualFixtureNumber: null,
		name: `Atomic Wash ${fixtureNumber}`,
		profileId: PROFILE_ID,
		profileRevision: 3,
		modeId: MODE_ID,
		splitPatches: [{ split: 1, universe: 1, address: fixtureNumber }],
		layerId: "default",
		directControl: null,
		location: { x: 0, y: 0, z: 0 },
		rotation: { x: 0, y: 0, z: 0 },
		logicalHeads: heads.map((head, headIndex) => ({
			profileHeadId: null,
			headIndex,
			fixtureId: head,
		})),
		multipatch: [],
		moveInBlackEnabled: true,
		moveInBlackDelayMillis: 0,
		highlightOverrides: [],
	};
}

/** Builds a store holding exactly these fixtures at patch revision 1. */
function storeWith(projections: PatchFixtureProjection[]): PatchStore {
	const store = new PatchStore(SHOW_ID, () => null, []);
	store.applySnapshot({
		showId: SHOW_ID,
		showRevision: 1,
		patchRevision: 1,
		cursor: 10,
		fixtures: projections,
		profileRevisions: [profileProjection()],
	});
	return store;
}

function applyDelta(
	store: PatchStore,
	patchRevision: number,
	projections: PatchFixtureProjection[],
	removed: string[] = [],
) {
	act(() => {
		store.applyDelta({
			showId: SHOW_ID,
			showRevision: patchRevision,
			patchRevision,
			eventSequence: 10 + patchRevision,
			fixtures: projections,
			removedFixtureIds: removed,
			profileRevisions: [profileProjection()],
		});
	});
}

describe("Patch scoped read selectors", () => {
	afterEach(() => {
		storeRef.current = null;
	});

	it("does not rerender a reader when an unrelated fixture changes", () => {
		const store = storeWith([
			fixtureProjection("fixture-a", 1),
			fixtureProjection("fixture-b", 2),
		]);
		storeRef.current = store;
		let renders = 0;
		let observed: readonly PatchedFixture[] = [];
		function Reader() {
			renders += 1;
			observed = usePatchFixturesById(["fixture-a"]);
			return null;
		}
		render(<Reader />);
		expect(renders).toBe(1);
		expect(observed.map((fixture) => fixture.fixture_id)).toEqual(["fixture-a"]);
		const before = observed;

		applyDelta(store, 2, [fixtureProjection("fixture-b", 22)]);

		expect(renders).toBe(1);
		expect(observed).toBe(before);
	});

	it("rerenders a reader when its own fixture changes", () => {
		const store = storeWith([
			fixtureProjection("fixture-a", 1),
			fixtureProjection("fixture-b", 2),
		]);
		storeRef.current = store;
		let renders = 0;
		const observed: { current: PatchedFixture | null } = { current: null };
		function Reader() {
			renders += 1;
			observed.current = usePatchFixture("fixture-a");
			return null;
		}
		render(<Reader />);
		expect(renders).toBe(1);

		applyDelta(store, 2, [fixtureProjection("fixture-a", 11)]);

		expect(renders).toBe(2);
		expect(observed.current?.fixture_number).toBe(11);
	});

	it("keeps the whole-list reader stable when no fixture identity changes", () => {
		const store = storeWith([fixtureProjection("fixture-a", 1)]);
		storeRef.current = store;
		let renders = 0;
		let observed: readonly PatchedFixture[] = [];
		function Reader() {
			renders += 1;
			observed = usePatchedFixtures();
			return null;
		}
		render(<Reader />);
		const before = observed;

		// An empty delta advances the revision chain without touching any fixture.
		applyDelta(store, 2, []);

		expect(renders).toBe(1);
		expect(observed).toBe(before);
	});

	it("resolves a selection made through a logical head exactly once", () => {
		const store = storeWith([
			fixtureProjection("fixture-a", 1, ["head-a1", "head-a2"]),
			fixtureProjection("fixture-b", 2),
		]);
		storeRef.current = store;
		let observed: readonly PatchedFixture[] = [];
		function Reader() {
			observed = usePatchFixturesForSelection(["head-a1", "head-a2"]);
			return null;
		}
		render(<Reader />);

		expect(observed.map((fixture) => fixture.fixture_id)).toEqual(["fixture-a"]);
	});

	it("ignores caller array identity churn for the same requested identities", () => {
		const store = storeWith([fixtureProjection("fixture-a", 1)]);
		storeRef.current = store;
		let renders = 0;
		let observed: readonly PatchedFixture[] = [];
		function Reader({ ids }: { ids: string[] }) {
			renders += 1;
			observed = usePatchFixturesById(ids);
			return null;
		}
		const view = render(<Reader ids={["fixture-a"]} />);
		const before = observed;

		view.rerender(<Reader ids={["fixture-a"]} />);

		expect(renders).toBe(2);
		expect(observed).toBe(before);
	});

	it("registers no store listener while a reader is disabled", () => {
		const store = storeWith([fixtureProjection("fixture-a", 1)]);
		const subscribe = vi.spyOn(store, "subscribe");
		storeRef.current = store;
		function Reader() {
			usePatchedFixtures(false);
			return null;
		}
		render(<Reader />);

		expect(subscribe).not.toHaveBeenCalled();
	});

	it("reports no authority and no fixtures outside a mounted Patch boundary", () => {
		storeRef.current = null;
		const observed: {
			fixtures: readonly PatchedFixture[];
			status: ReturnType<typeof usePatchStatus> | null;
		} = { fixtures: [], status: null };
		function Reader() {
			observed.fixtures = usePatchedFixtures();
			observed.status = usePatchStatus();
			return null;
		}
		render(<Reader />);

		expect(observed.fixtures).toEqual([]);
		expect(observed.status).toEqual({ status: "loading", error: null });
	});
});
