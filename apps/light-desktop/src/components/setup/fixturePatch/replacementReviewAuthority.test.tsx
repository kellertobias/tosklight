import {
	act,
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type {
	PatchFixtureProjection,
	PatchMutationOutcome,
	PatchSnapshot,
} from "../../../features/patch/contracts";
import { PATCH_OBJECT_CHANGED_EVENT } from "../../../features/patch/externalRepair";
import { PatchViewProvider } from "../../../features/patch/PatchContext";
import {
	type PatchTransport,
	PatchTransportError,
} from "../../../features/patch/transport";
import {
	blankFixtureProfile,
	fixtureDefinitionKey,
	fixtureDefinitionsFromProfiles,
} from "../fixtureProfileModel";
import {
	type PatchController,
	PatchControllerProvider,
	usePatchController,
} from "./controller";
import { saveEdit } from "./editSave";
import { armEdit, cancelEdit } from "./editSession";
import { FixtureEditDialog } from "./PatchEditSurfaces";

const server = vi.hoisted(() => ({
	fixtureProfiles: [] as unknown[],
	fixtureLibrary: [],
	patchLayers: [],
}));
vi.mock("../../../features/fixtureLibrary/FixtureLibraryContext", () => ({
	useFixtureLibrary: () => server,
}));
vi.mock("../../../state/AppContext", () => ({
	useApp: () => ({ state: { patchSetArmed: true }, dispatch: vi.fn() }),
}));
vi.mock("../../../features/stageLayout/StageLayoutState", () => ({
	useStagePositions3d: () => ({}),
}));
vi.mock("../../../features/stageLayout/StageLayoutActions", () => ({
	useStageLayoutActions: () => ({ canWrite: false }),
}));
vi.mock("./selection", () => ({
	usePatchSelection: () => ({ selected: [], actions: null }),
	fixtureSelectionIds: (f: { fixture_id: string }) => [f.fixture_id],
}));
afterEach(cleanup);
const SHOW = "10000000-0000-4000-8000-000000000001";
const FIXTURE = "20000000-0000-4000-8000-000000000001";
function deferred<T>() {
	let resolve!: (v: T) => void;
	let reject!: (e: Error) => void;
	const promise = new Promise<T>((a, b) => {
		resolve = a;
		reject = b;
	});
	void promise.catch(() => undefined);
	return { promise, resolve, reject };
}

async function mountReview(refresh?: Promise<PatchSnapshot>) {
	const profile = blankFixtureProfile();
	profile.name = "Original wash";
	const target = blankFixtureProfile();
	target.name = "Replacement wash";
	const definitions = fixtureDefinitionsFromProfiles([profile, target]);
	server.fixtureProfiles = [profile, target];
	const fixture: PatchFixtureProjection = {
		fixtureId: FIXTURE,
		fixtureRevision: 1,
		fixtureNumber: 1,
		virtualFixtureNumber: null,
		name: "Original fixture",
		profileId: profile.id,
		profileRevision: profile.revision,
		modeId: profile.modes[0].id,
		splitPatches: [{ split: 1, universe: 1, address: 1 }],
		layerId: "default",
		directControl: null,
		location: { x: 10, y: 20, z: 30 },
		rotation: { x: 0, y: 0, z: 0 },
		logicalHeads: [],
		multipatch: [],
		moveInBlackEnabled: true,
		moveInBlackDelayMillis: 0,
		highlightOverrides: [],
	};
	const profiles = [profile, target].map((p) => ({
		profileId: p.id,
		profileRevision: p.revision,
		contentDigest: "test-profile",
		manufacturer: p.manufacturer,
		name: p.name,
		fixtureType: p.fixture_type,
		patchPolicy: "dmx" as const,
		profileSnapshot: p,
		referencedModes: p.modes.map((m) => ({
			modeId: m.id,
			name: m.name,
			splits: m.splits.map((s) => ({
				split: s.number,
				footprint: s.footprint,
			})),
		})),
	}));
	const initial: PatchSnapshot = {
		showId: SHOW,
		showRevision: 50,
		patchRevision: 4,
		cursor: 10,
		fixtures: [fixture],
		profileRevisions: profiles,
	};
	let currentDocument = 53;
	let refreshed: PatchSnapshot = { ...initial, showRevision: 53, cursor: 11 };
	let reads = 0;
	const snapshot = vi.fn(() => {
		reads++;
		return reads === 1
			? Promise.resolve(initial)
			: reads === 2 && refresh
				? refresh
				: Promise.resolve(refreshed);
	});
	const update = vi
		.fn<NonNullable<PatchTransport["patchFixtureUpdate"]>>()
		.mockImplementation(
			async (
				showId,
				fixtureId,
				fixtureRevision,
				patchRevision,
				showRevision,
				requestId,
				_instance,
				action,
			) => {
				if (showRevision !== currentDocument)
					throw new PatchTransportError(
						"stale show revision",
						409,
						currentDocument,
						false,
					);
				if (action.type !== "replace_profile")
					throw new Error("Expected replacement intent");
				return {
					showId,
					showRevision: currentDocument + 1,
					patchRevision: patchRevision + 1,
					eventSequence: 12,
					requestId,
					replayed: false,
					changed: true,
					fixtures: [
						{
							...fixture,
							fixtureId,
							fixtureRevision: fixtureRevision + 1,
							profileId: action.profileId,
							profileRevision: action.profileRevision,
							modeId: action.modeId,
						},
					],
					profileRevisions: [],
					removedFixtureIds: [],
				} satisfies PatchMutationOutcome;
			},
		);
	const transport: PatchTransport = {
		snapshot,
		patchFixtures: vi.fn(),
		patchFixtureUpdate: update,
		subscribe: () => ({ repair: vi.fn(), close: vi.fn() }),
	};
	let controller!: PatchController;
	function Probe() {
		controller = usePatchController();
		return (
			<>
				<button
					type="button"
					onClick={() =>
						armEdit(controller, controller.patch.fixtures[0], "mode")
					}
				>
					Review replacement
				</button>
				<button
					type="button"
					onClick={() => {
						controller.ui.setReplacingFixture(true);
						const d = definitions.find(
							(d) => d.profile_snapshot?.id === target.id,
						);
						if (!d) throw new Error("Replacement definition missing");
						const f = controller.data.families.find((f) =>
							f.modes.some(
								(m) => fixtureDefinitionKey(m) === fixtureDefinitionKey(d),
							),
						);
						if (!f) throw new Error("Replacement family missing");
						controller.ui.setFamilyKey(f.key);
						controller.ui.setDefinitionKey(fixtureDefinitionKey(d));
					}}
				>
					Choose target
				</button>
				<button type="button" onClick={() => saveEdit(controller)}>
					Submit replacement intent
				</button>
				<button type="button" onClick={() => cancelEdit(controller)}>
					Cancel review
				</button>
				<FixtureEditDialog />
			</>
		);
	}
	const view = render(
		<PatchViewProvider
			showId={SHOW}
			initialFixtures={[]}
			definitions={definitions}
			transport={transport}
		>
			<PatchControllerProvider>
				<Probe />
			</PatchControllerProvider>
		</PatchViewProvider>,
	);
	await waitFor(() => expect(controller.patch.status).toBe("ready"));
	return {
		snapshot,
		update,
		fixture,
		profile,
		initial,
		latest: () => controller,
		view,
		setDocument: (v: number) => {
			currentDocument = v;
			refreshed = {
				...refreshed,
				showRevision: v,
				cursor: refreshed.cursor + 1,
			};
		},
		setRefreshed: (v: PatchSnapshot) => {
			refreshed = v;
		},
	};
}

it("refreshes before review and submits the actual current revision after a nonpatch portable commit", async () => {
	const rig = await mountReview();
	fireEvent.click(screen.getByText("Review replacement"));
	await waitFor(() =>
		expect(rig.latest().ui.replacementRevision).not.toBeNull(),
	);
	expect(rig.latest().data.selected?.location).toEqual(rig.fixture.location);
	fireEvent.click(screen.getByText("Choose target"));
	fireEvent.click(screen.getByText("Submit replacement intent"));
	await waitFor(() => expect(rig.update).toHaveBeenCalledTimes(1));
	expect(rig.update.mock.calls[0].slice(2, 5)).toEqual([1, 4, 53]);
	expect(rig.snapshot).toHaveBeenCalledTimes(2);
	await waitFor(() => expect(rig.latest().ui.edit).toBeNull());
});
it("keeps post-review revision changes rejected without rebasing or repeating the replacement", async () => {
	const rig = await mountReview();
	fireEvent.click(screen.getByText("Review replacement"));
	await waitFor(() =>
		expect(rig.latest().ui.replacementRevision).toEqual({ show: 53, patch: 4 }),
	);
	rig.setDocument(54);
	fireEvent.click(screen.getByText("Choose target"));
	fireEvent.click(screen.getByText("Submit replacement intent"));
	await waitFor(() =>
		expect(rig.latest().ui.editError).toBe("stale show revision"),
	);
	expect(rig.update).toHaveBeenCalledTimes(1);
	expect(rig.update.mock.calls[0][4]).toBe(53);
	expect(rig.latest().ui.replacementRevision).toEqual({ show: 53, patch: 4 });
	expect(rig.latest().ui.edit).toBe("mode");
	expect(rig.latest().patch.showRevision).toBe(54);
});
it("shows pending refresh, hides stale review controls and ignores completion after Cancel", async () => {
	const gate = deferred<PatchSnapshot>();
	const rig = await mountReview(gate.promise);
	fireEvent.click(screen.getByText("Review replacement"));
	expect(screen.getByRole("status")).toHaveTextContent(/loading.*replacement/i);
	expect(
		screen.queryByText("Replace fixture with another product"),
	).not.toBeInTheDocument();
	for (const button of screen.getAllByRole("button", { name: "Set" }))
		expect(button).toBeDisabled();
	fireEvent.click(screen.getByText("Cancel review"));
	await act(async () => gate.resolve({ ...rig.initial, showRevision: 53 }));
	expect(rig.latest().ui.edit).toBeNull();
	expect(rig.latest().ui.replacementRevision).toBeNull();
	expect(rig.update).not.toHaveBeenCalled();
});
it("shows actionable refresh failure and never permits Set with old consent", async () => {
	const gate = deferred<PatchSnapshot>();
	const rig = await mountReview(gate.promise);
	fireEvent.click(screen.getByText("Review replacement"));
	await act(async () => gate.reject(new Error("Snapshot unavailable")));
	expect(await screen.findByRole("alert")).toHaveTextContent(
		/Snapshot unavailable/,
	);
	for (const button of screen.getAllByRole("button", { name: "Set" }))
		expect(button).toBeDisabled();
	expect(rig.latest().ui.replacementRevision).toBeNull();
	expect(rig.update).not.toHaveBeenCalled();
});

it("reviews source profile and pose from the refreshed snapshot and ignores late completion on unmount", async () => {
	const rig = await mountReview();
	const profile = structuredClone(rig.profile);
	profile.revision++;
	const refreshed = {
		...rig.initial,
		showRevision: 53,
		patchRevision: 5,
		fixtures: [
			{
				...rig.fixture,
				profileRevision: profile.revision,
				location: { x: 900, y: 800, z: 700 },
			},
		],
		profileRevisions: rig.initial.profileRevisions.map((p) =>
			p.profileId === profile.id
				? { ...p, profileRevision: profile.revision, profileSnapshot: profile }
				: p,
		),
	};
	rig.setRefreshed(refreshed);
	fireEvent.click(screen.getByText("Review replacement"));
	await waitFor(() =>
		expect(rig.latest().ui.replacementRevision).toEqual({ show: 53, patch: 5 }),
	);
	expect(
		rig.latest().data.selected?.definition.profile_snapshot?.revision,
	).toBe(profile.revision);
	expect(rig.latest().data.selected?.location).toEqual(
		refreshed.fixtures[0].location,
	);
	rig.view.unmount();
	const gate = deferred<PatchSnapshot>();
	const late = await mountReview(gate.promise);
	fireEvent.click(screen.getByText("Review replacement"));
	const token = late.latest().ui.replacementReviewRequest.current;
	late.view.unmount();
	await act(async () => gate.resolve({ ...late.initial, showRevision: 53 }));
	expect(late.latest().ui.replacementReviewRequest.current).toBeGreaterThan(
		token,
	);
	expect(late.update).not.toHaveBeenCalled();
});

it("waits an older in-flight repair then starts a fresh review snapshot after the operator request", async () => {
	const oldRead = deferred<PatchSnapshot>();
	const rig = await mountReview(oldRead.promise);
	act(() =>
		window.dispatchEvent(
			new CustomEvent(PATCH_OBJECT_CHANGED_EVENT, { detail: { showId: SHOW } }),
		),
	);
	await waitFor(() => expect(rig.snapshot).toHaveBeenCalledTimes(2));
	rig.setRefreshed({ ...rig.initial, showRevision: 53, cursor: 12 });
	fireEvent.click(screen.getByText("Review replacement"));
	await act(async () => oldRead.resolve(rig.initial));
	await waitFor(() =>
		expect(rig.latest().ui.replacementRevision).toEqual({ show: 53, patch: 4 }),
	);
	expect(rig.snapshot).toHaveBeenCalledTimes(3);
});

it("keeps removed-fixture refresh failure visible without stale review or mutation", async () => {
	const rig = await mountReview();
	rig.setRefreshed({ ...rig.initial, showRevision: 53, fixtures: [] });
	fireEvent.click(screen.getByText("Review replacement"));
	expect(await screen.findByRole("alert")).toHaveTextContent(
		"This fixture is no longer in the show",
	);
	expect(
		screen.queryByText("Replace fixture with another product"),
	).not.toBeInTheDocument();
	for (const button of screen.getAllByRole("button", { name: "Set" }))
		expect(button).toBeDisabled();
	expect(rig.latest().ui.replacementRevision).toBeNull();
	expect(rig.latest().patch.fixtures).toHaveLength(0);
	expect(rig.update).not.toHaveBeenCalled();
});
