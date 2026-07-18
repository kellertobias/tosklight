import type { PatchedFixture } from "../../api/types";
import type { ServerController } from "./model";
import type { ServerContextValue } from "./ServerContextValue";

export function createPatchActions(
	model: ServerController,
): Pick<
	ServerContextValue,
	| "patchFixture"
	| "updatePatchedFixture"
	| "deletePatchedFixture"
	| "savePatchLayer"
> {
	const {
		client,
		setError,
		bootstrap,
		session,
		patchLayers,
		loadShowObjects,
		refresh,
	} = model;
	return {
		patchFixture: async (input) => {
			try {
				if (!bootstrap?.active_show)
					throw new Error("No active show is available");
				if ((input.universe == null) !== (input.address == null))
					throw new Error(
						"Universe and address must both be set or both be empty",
					);
				if (
					input.universe != null &&
					input.address != null &&
					(input.universe < 1 ||
						input.address < 1 ||
						input.address + input.definition.footprint - 1 > 512)
				)
					throw new Error(
						"The fixture must fit within universe addresses 1–512",
					);
				const fixture_id = crypto.randomUUID();
				const body = {
					fixture_id,
					fixture_number: input.fixture_number,
					virtual_fixture_number: input.virtual_fixture_number ?? null,
					name: input.name,
					definition: input.definition,
					universe: input.universe,
					address: input.address,
					split_patches: input.split_patches ?? [],
					highlight_overrides: {},
					layer_id: input.layer_id ?? "default",
					direct_control: null,
					location: { x: 0, y: 0, z: 0 },
					rotation: { x: 0, y: 0, z: 0 },
					logical_heads: [],
					multipatch: [],
				};
				await client.putObject(
					bootstrap.active_show.id,
					"patched_fixture",
					fixture_id,
					body,
					0,
				);
				await refresh();
				setError(null);
				return fixture_id;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				return null;
			}
		},
		updatePatchedFixture: async (fixtureId, changes) => {
			try {
				if (!bootstrap?.active_show)
					throw new Error("No active show is available");
				const objects = await client.objects<PatchedFixture>(
					bootstrap.active_show.id,
					"patched_fixture",
				);
				const object = objects.find((candidate) => candidate.id === fixtureId);
				if (!object) throw new Error("Patched fixture object was not found");
				await client.putObject(
					bootstrap.active_show.id,
					"patched_fixture",
					fixtureId,
					{ ...object.body, ...changes },
					object.revision,
				);
				await refresh();
				setError(null);
				return true;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				return false;
			}
		},
		deletePatchedFixture: async (fixtureId) => {
			try {
				if (!bootstrap?.active_show)
					throw new Error("No active show is available");
				const objects = await client.objects<PatchedFixture>(
					bootstrap.active_show.id,
					"patched_fixture",
				);
				const object = objects.find((candidate) => candidate.id === fixtureId);
				if (!object) throw new Error("Patched fixture object was not found");
				await client.deleteObject(
					bootstrap.active_show.id,
					"patched_fixture",
					fixtureId,
					object.revision,
				);
				await refresh();
				setError(null);
				return true;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				return false;
			}
		},
		savePatchLayer: async (layer) => {
			try {
				if (!bootstrap?.active_show)
					throw new Error("No active show is available");
				const existing = patchLayers.find((item) => item.id === layer.id);
				await client.putObject(
					bootstrap.active_show.id,
					"patch_layer",
					layer.id,
					layer,
					existing?.revision ?? 0,
				);
				await loadShowObjects(
					bootstrap.active_show.id,
					session?.user.id ?? null,
				);
				setError(null);
				return true;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				return false;
			}
		},
	};
}
