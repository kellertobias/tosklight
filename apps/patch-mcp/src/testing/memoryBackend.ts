/** A backend that answers from memory and records what was written, for the tool tests. */

import {
	type FixtureRef,
	findFixture,
	type MediaLayoutWire,
	type PatchBackend,
	type PatchedFixture,
} from "../backend";

const COLLECTIONS: Record<string, string> = {
	media_fallback_asset: "fallbackAssets",
	media_server: "servers",
	media_source: "sources",
	led_module_type: "ledModuleTypes",
	media_surface: "surfaces",
	media_projector: "projectors",
};

export function memoryBackend(
	options: {
		fixtures?: Array<Partial<PatchedFixture>>;
		profiles?: Array<Record<string, unknown>>;
		media?: Partial<MediaLayoutWire>;
		product?: string;
		withMedia?: boolean;
	} = {},
) {
	const state = {
		patch_revision: 3,
		fixtures: (options.fixtures ?? []).map((fixture, index) => ({
			fixture_id: `id-${index + 1}`,
			fixture_number: index + 1,
			virtual_fixture_number: null,
			name: `Fixture ${index + 1}`,
			profile_id: "dimmer",
			profile_revision: 1,
			mode_id: "dm",
			layer_id: "default",
			split_patches: [{ split: 1, universe: 1, address: index + 1 }],
			...fixture,
		})) as PatchedFixture[],
	};
	const writes: Array<{ revision: number; fixtures: PatchedFixture[]; removed: string[] }> = [];
	const intents: Array<{ kind: string; id: string; intent: any }> = [];
	const layout: MediaLayoutWire = {
		fallbackAssets: [],
		servers: [],
		sources: [],
		ledModuleTypes: [],
		surfaces: [],
		projectors: [],
		...(options.media as MediaLayoutWire | undefined),
	};

	const fixture = async (ref: FixtureRef) => {
		const snapshot = structuredClone(state);
		const found = findFixture(snapshot, ref);
		if (!found) throw new Error(`no fixture numbered ${ref}`);
		return { snapshot, fixture: found };
	};
	const putFixtures = async (revision: number, fixtures: PatchedFixture[], removed: string[] = []) => {
		writes.push({ revision, fixtures: structuredClone(fixtures), removed });
		for (const written of fixtures) {
			const index = state.fixtures.findIndex((each) => each.fixture_id === written.fixture_id);
			if (index >= 0) state.fixtures[index] = structuredClone(written);
			else state.fixtures.push(structuredClone(written));
		}
		state.fixtures = state.fixtures.filter((each) => !removed.includes(each.fixture_id));
		state.patch_revision += 1;
	};
	const backend: PatchBackend = {
		product: options.product ?? "The Architect",
		patch: async () => structuredClone(state),
		fixture,
		putFixtures,
		async editFixture(ref, change) {
			const found = await fixture(ref);
			const edited = change(structuredClone(found.fixture));
			await putFixtures(found.snapshot.patch_revision, [edited]);
			return edited;
		},
		profiles: async () => ({ profiles: structuredClone(options.profiles ?? []) }),
		layers: async () => [],
		layer: async () => null,
	};
	if (options.withMedia !== false) {
		backend.mediaLayout = async () => structuredClone(layout);
		backend.applyMediaIntent = async (kind, id, intent) => {
			intents.push({ kind, id, intent: structuredClone(intent) });
			const collection = layout[COLLECTIONS[kind]];
			const index = collection.findIndex((entry) => entry.object.body.id === id);
			const current = index >= 0 ? collection[index].revision : 0;
			if (current !== intent.expectedRevision)
				throw new Error(`${kind} ${id} revision conflict: expected ${intent.expectedRevision}, current ${current}`);
			const action = intent.action as any;
			if (action.type === "delete") collection.splice(index, 1);
			else {
				const entry = { object: structuredClone(action.object), revision: current + 1 };
				if (index >= 0) collection[index] = entry;
				else collection.push(entry);
			}
			return {
				requestId: String(intent.requestId),
				replayed: false,
				changed: true,
				snapshot: structuredClone(layout),
			};
		};
	}
	return { backend, state, writes, intents, layout };
}
