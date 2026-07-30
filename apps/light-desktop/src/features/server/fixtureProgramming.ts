import type { StoredPreset } from "../../api/types";
import type { ServerCapabilities } from "./capabilityContracts";
import type { ServerController } from "./model";

/** Compatibility surface for transient fixture actions and preset generation only. */
export function createFixtureProgrammingActions(
	model: ServerController,
): Pick<
	ServerCapabilities,
	"controlFixtureAction" | "controlFixtureActions" | "generateFixturePresets"
> {
	const { api, setError, bootstrap } = model;
	return {
		controlFixtureAction: async (fixtureId, actionId, active) => {
			try {
				await api.programming.controlFixtureAction(fixtureId, actionId, active);
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		controlFixtureActions: async (
			targets,
			expectedSelectionRevision,
			active,
		) => {
			try {
				await api.programming.controlFixtureActions(
					targets,
					expectedSelectionRevision,
					active,
				);
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		generateFixturePresets: async (fixtureIds) => {
			try {
				if (!bootstrap?.active_show)
					throw new Error("Open a show before generating presets");
				const showId = bootstrap.active_show.id;
				let authority = model.showObjectsStore.getSnapshot();
				if (authority.showId !== showId || authority.showRevision == null) {
					const hydrated =
						await api.showObjects.collectionSnapshot<StoredPreset>(
							showId,
							"preset",
						);
					model.showObjectsStore.setCollection(
						showId,
						"preset",
						hydrated.objects,
						undefined,
						hydrated.showRevision,
					);
					authority = model.showObjectsStore.getSnapshot();
				}
				if (authority.showId !== showId || authority.showRevision == null)
					throw new Error("Preset authority is not ready");
				const result = await api.programming.generateFixturePresets(
					fixtureIds,
					authority.showRevision,
				);
				model.showObjectsStore.installShowRevision(showId, result.showRevision);
				setError(null);
				return result;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				return null;
			}
		},
	};
}
