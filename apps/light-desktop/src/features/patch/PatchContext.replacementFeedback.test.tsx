import { act, cleanup, render, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type { PatchController } from "../../components/setup/fixturePatch/controller";
import { saveEdit } from "../../components/setup/fixturePatch/editSave";
import {
	blankFixtureProfile,
	fixtureDefinitionsFromProfiles,
} from "../../components/setup/fixtureProfileModel";
import type { PatchFixtureProjection, PatchSnapshot } from "./contracts";
import {
	type PatchContextValue,
	PatchViewProvider,
	usePatch,
	usePatchView,
} from "./PatchContext";
import { type PatchTransport, PatchTransportError } from "./transport";

afterEach(cleanup);

const SHOW = "10000000-0000-4000-8000-000000000001";
const FIXTURE = "20000000-0000-4000-8000-000000000001";

async function mountedPatch(rejection: Error) {
	const profile = blankFixtureProfile();
	const definition = fixtureDefinitionsFromProfiles([profile])[0];
	const modeId = profile.modes[0].id;
	const fixture: PatchFixtureProjection = {
		fixtureId: FIXTURE,
		fixtureRevision: 1,
		fixtureNumber: 9015,
		virtualFixtureNumber: null,
		name: "Replacement feedback fixture",
		profileId: profile.id,
		profileRevision: profile.revision,
		modeId,
		splitPatches: [{ split: 1, universe: 50, address: 1 }],
		layerId: "default",
		directControl: null,
		location: { x: 0, y: 0, z: 0 },
		rotation: { x: 0, y: 0, z: 0 },
		logicalHeads: [],
		multipatch: [],
		moveInBlackEnabled: true,
		moveInBlackDelayMillis: 0,
		highlightOverrides: [],
	};
	const snapshot: PatchSnapshot = {
		showId: SHOW,
		showRevision: 489,
		patchRevision: 4,
		cursor: 10,
		fixtures: [fixture],
		profileRevisions: [
			{
				profileId: profile.id,
				profileRevision: profile.revision,
				contentDigest: "test-profile",
				manufacturer: profile.manufacturer,
				name: profile.name,
				fixtureType: profile.fixture_type,
				patchPolicy: "dmx",
				profileSnapshot: profile,
				referencedModes: [
					{ modeId, name: "Test mode", splits: [{ split: 1, footprint: 1 }] },
				],
			},
		],
	};
	const update = vi.fn().mockRejectedValue(rejection);
	const transport: PatchTransport = {
		snapshot: async () => snapshot,
		patchFixtures: vi.fn(),
		patchFixtureUpdate: update,
		subscribe: () => ({ repair: vi.fn(), close: vi.fn() }),
	};
	let current: PatchContextValue | undefined;
	function Probe() {
		usePatchView();
		current = usePatch();
		return null;
	}
	render(
		<PatchViewProvider
			showId={SHOW}
			definitions={[definition]}
			initialFixtures={[]}
			transport={transport}
		>
			<Probe />
		</PatchViewProvider>,
	);
	await waitFor(() => expect(current?.status).toBe("ready"));
	if (!current) throw new Error("Missing ready Patch context");
	return {
		captured: current,
		definition,
		profile,
		modeId,
		fixture,
		update,
		latest: () => current,
	};
}

it.each([
	new PatchTransportError(
		"Replacement consent is stale. Reopen Product / mode and review the mapping.",
		409,
		5,
		false,
	),
	new PatchTransportError(
		"Replacement head mapping contains an unknown target head.",
		400,
		4,
		false,
	),
	new PatchTransportError(
		"Replacement transaction failed: database unavailable",
		500,
		null,
		false,
	),
])("shows the exact request rejection despite a captured empty context error: $message", async (rejection) => {
	const { captured, definition, profile, modeId, update, latest } =
		await mountedPatch(rejection);
	expect(captured.error).toBeNull();
	const error = vi.fn();
	const setEdit = vi.fn();
	const controller = {
		data: {
			selected: captured.fixtures[0],
			all: captured.fixtures,
			definition,
		},
		ui: {
			edit: "mode",
			replacingFixture: true,
			replacementRevision: { show: 489, patch: 4 },
			replacementHeads: {},
			setEditError: error,
			setEdit,
		},
		patch: captured,
	} as unknown as PatchController;
	await act(async () => {
		saveEdit(controller);
	});
	await waitFor(() =>
		expect(error).toHaveBeenLastCalledWith(rejection.message),
	);
	expect(update).toHaveBeenCalledTimes(rejection.status >= 500 ? 2 : 1);
	if (rejection.status >= 500)
		expect(update.mock.calls[1][5]).toBe(update.mock.calls[0][5]);
	expect(captured.error).toBeNull();
	expect(setEdit).not.toHaveBeenCalled();
	expect(latest()?.fixtures[0]).toEqual(captured.fixtures[0]);
	// Existing callers retain a boolean outcome and do not gain a throwing contract.
	await act(async () => {
		await expect(
			captured.updateFixtureIntent(FIXTURE, null, {
				type: "replace_profile",
				profileId: profile.id,
				profileRevision: profile.revision,
				modeId,
				headMapping: [],
				expectedShowRevision: 489,
				expectedPatchRevision: 4,
			}),
		).resolves.toBe(false);
	});
});

it("retains the successful boolean outcome without reporting a rejection", async () => {
	const { captured, profile, modeId, fixture, update } = await mountedPatch(
		new Error("unused"),
	);
	update.mockImplementation(
		async (
			_show,
			_fixture,
			_fixtureRevision,
			_patchRevision,
			_showRevision,
			requestId,
		) => ({
			showId: SHOW,
			showRevision: 490,
			patchRevision: 5,
			eventSequence: 11,
			requestId,
			changed: true,
			replayed: false,
			fixtures: [{ ...fixture, fixtureRevision: 2 }],
			removedFixtureIds: [],
			profileRevisions: [],
		}),
	);
	const onRejected = vi.fn();
	await act(async () => {
		await expect(
			captured.updateFixtureIntent(
				FIXTURE,
				null,
				{
					type: "replace_profile",
					profileId: profile.id,
					profileRevision: profile.revision,
					modeId,
					headMapping: [],
					expectedShowRevision: 489,
					expectedPatchRevision: 4,
				},
				onRejected,
			),
		).resolves.toBe(true);
	});
	expect(onRejected).not.toHaveBeenCalled();
});
