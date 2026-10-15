// @vitest-environment jsdom
import { act, cleanup, render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type {
	DynamicSpatialMappingOverrideProjection,
	DynamicSpatialPreviewResponse,
} from "../../api/types";
import {
	blankFixtureProfile,
	fixtureDefinitionsFromProfiles,
} from "../../components/setup/fixtureProfileModel";
import { PatchViewProvider } from "../../features/patch/PatchContext";
import type {
	PatchFixtureProjection,
	PatchSnapshot,
} from "../../features/patch/contracts";
import type {
	PatchEventObserver,
	PatchTransport,
} from "../../features/patch/transport";
import type { ShowObject } from "../../features/showObjects/contracts";
import { AppProvider } from "../../state/AppContext";
import {
	createDefaultDynamicDefinition,
	DynamicEditor,
	type DynamicEditorView,
} from "./DynamicsEditor";

beforeEach(() => {
	const entries = new Map<string, string>();
	vi.stubGlobal("localStorage", {
		getItem: (key: string) => entries.get(key) ?? null,
		setItem: (key: string, value: string) => {
			entries.set(key, String(value));
		},
		removeItem: (key: string) => {
			entries.delete(key);
		},
		clear: () => entries.clear(),
		key: (index: number) => [...entries.keys()][index] ?? null,
		get length() {
			return entries.size;
		},
	});
});
afterEach(() => {
	cleanup();
	vi.unstubAllGlobals();
});

it("owns the Patch stream only in Projection and refreshes unchanged Dynamic dots from a location-only Patch event", async () => {
	const showId = "10000000-0000-4000-8000-000000000001";
	const fixtureId = "20000000-0000-4000-8000-000000000001";
	const profile = blankFixtureProfile();
	const definitions = fixtureDefinitionsFromProfiles([profile]);
	const modeId = profile.modes[0].id;
	const fixture: PatchFixtureProjection = {
		fixtureId,
		fixtureRevision: 1,
		fixtureNumber: 1,
		virtualFixtureNumber: null,
		name: "Position witness",
		profileId: profile.id,
		profileRevision: profile.revision,
		modeId,
		splitPatches: [{ split: 1, universe: 50, address: 1 }],
		layerId: "default",
		directControl: null,
		location: { x: 1000, y: 2000, z: 3000 },
		rotation: { x: 0, y: 0, z: 0 },
		logicalHeads: [],
		multipatch: [],
		moveInBlackEnabled: true,
		moveInBlackDelayMillis: 0,
		highlightOverrides: [],
	};
	const snapshot: PatchSnapshot = {
		showId,
		showRevision: 1,
		patchRevision: 1,
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
	let observer: PatchEventObserver | undefined;
	const close = vi.fn();
	const subscribe = vi.fn(
		(_showId: string, _cursor: number, next: PatchEventObserver) => {
			observer = next;
			return { close, repair: vi.fn() };
		},
	);
	const snapshotRead = vi.fn(async () => snapshot);
	const transport: PatchTransport = {
		snapshot: snapshotRead,
		subscribe,
		patchFixtures: vi.fn(),
	};
	const body = createDefaultDynamicDefinition(149, "intensity");
	body.spatial_mapping = {
		projection: {
			type: "replace",
			value: {
				kind: "planar",
				preset: "top",
				anchor: { x: 0, y: 0, z: 0 },
				view_direction: { x: 0, y: 0, z: -1 },
				rotation_degrees: 0,
			},
		},
		shape: { type: "inherit" },
	};
	const dynamic: ShowObject<"dynamic"> = {
		kind: "dynamic",
		id: body.id,
		revision: 7,
		updated_at: "2026-10-15T00:00:00Z",
		body,
	};
	let position = { x: 1, y: 2, z: 3 };
	const loadPreview = vi.fn(
		async (
			draft: DynamicSpatialMappingOverrideProjection,
		): Promise<DynamicSpatialPreviewResponse> => ({
			show_id: showId,
			show_revision: 1,
			dynamic_id: dynamic.id,
			dynamic_revision: dynamic.revision,
			target_binding: body.target_binding,
			base: { type: "targetless" },
			inherited_mapping: null,
			draft,
			source_order: [],
			ordered_fixture_ids: [],
			projected_positions: [],
			ranks: [],
			rank_count: 0,
			warnings: [],
			fixture_positions: [
				{ fixture_id: fixtureId, physical_fixture_id: fixtureId, position },
			],
		}),
	);
	const mutate = vi.fn(async () => undefined);
	const apply = vi.fn(async () => "applied" as const);
	const tree = (view: DynamicEditorView) => (
		<AppProvider>
			<PatchViewProvider
				showId={showId}
				initialFixtures={[]}
				definitions={definitions}
				transport={transport}
			>
				<DynamicEditor
					dynamic={dynamic}
					view={view}
					compact={false}
					busy={false}
					error={null}
					attributes={[]}
					presets={[]}
					runtime={null}
					selection={[fixtureId]}
					selectedGroupId={null}
					onBack={vi.fn()}
					onMutate={mutate}
					onLoadSpatialPreview={loadPreview}
					onApplySpatialMapping={apply}
					onDelete={vi.fn()}
					onMove={vi.fn()}
					onCopy={vi.fn()}
				/>
			</PatchViewProvider>
		</AppProvider>
	);
	const mounted = render(tree("curves"));
	await act(async () => undefined);
	expect(snapshotRead).not.toHaveBeenCalled();
	expect(subscribe).not.toHaveBeenCalled();
	mounted.rerender(tree("projection"));
	await waitFor(() => expect(subscribe).toHaveBeenCalledTimes(1));
	await waitFor(() =>
		expect(
			mounted.container.querySelectorAll(".projection-stage-fixture.selected"),
		).toHaveLength(1),
	);
	const initialDot = mounted.container.querySelector(
		".projection-stage-fixture.selected",
	);
	const before = [initialDot?.getAttribute("cx"), initialDot?.getAttribute("cy")];
	const priorCalls = loadPreview.mock.calls.length;
	position = { x: -3, y: -2, z: 1 };
	await act(async () => {
		observer?.message({
			type: "event",
			sequence: 11,
			change: {
				showId,
				showRevision: 2,
				patchRevision: 2,
				eventSequence: 11,
				fixtures: [
					{
						...fixture,
						fixtureRevision: 2,
						location: { x: -3000, y: -2000, z: 1000 },
					},
				],
				removedFixtureIds: [],
				profileRevisions: [],
			},
		});
	});
	await waitFor(() =>
		expect(loadPreview.mock.calls.length).toBeGreaterThan(priorCalls),
	);
	await waitFor(() => {
		const next = mounted.container.querySelector(
			".projection-stage-fixture.selected",
		);
		expect([next?.getAttribute("cx"), next?.getAttribute("cy")]).not.toEqual(before);
	});
	expect(dynamic.revision).toBe(7);
	expect(mutate).not.toHaveBeenCalled();
	expect(apply).not.toHaveBeenCalled();
	mounted.rerender(tree("curves"));
	await waitFor(() => expect(close).toHaveBeenCalledTimes(1));
	expect(snapshotRead).toHaveBeenCalledTimes(1);
});
