import type { ServerController } from "./model";
import type { ServerCapabilities } from "./capabilityContracts";

/** Compatibility surface for transient fixture actions and preset generation only. */
export function createFixtureProgrammingActions(
	model: ServerController,
): Pick<ServerCapabilities, "controlFixtureAction" | "generateFixturePresets"> {
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
		generateFixturePresets: async (fixtureIds) => {
			try {
				if (!bootstrap?.active_show)
					throw new Error("Open a show before generating presets");
				const result = await api.programming.generateFixturePresets(fixtureIds);
				setError(null);
				return result;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				return null;
			}
		},
	};
}
