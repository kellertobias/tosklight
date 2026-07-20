import type { ServerController } from "./model";
import type { ServerContextValue } from "./ServerContextValue";

export function createProgrammerValueActions(
	model: ServerController,
): Pick<
	ServerContextValue,
	| "setProgrammer"
	| "setProgrammerMany"
	| "controlFixtureAction"
	| "generateFixturePresets"
> {
	const { client, setError, bootstrap } = model;
	return {
		setProgrammer: async (fixtureId, attribute, level) => {
			try {
				await client.setProgrammer(fixtureId, attribute, level);
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		setProgrammerMany: async (assignments) => {
			try {
				await client.setProgrammerMany(assignments);
				setError(null);
				return true;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				return false;
			}
		},
		controlFixtureAction: async (fixtureId, actionId, active) => {
			try {
				await client.controlFixtureAction(fixtureId, actionId, active);
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		generateFixturePresets: async (fixtureIds) => {
			try {
				if (!bootstrap?.active_show)
					throw new Error("Open a show before generating presets");
				const result = await client.generateFixturePresets(fixtureIds);
				setError(null);
				return result;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				return null;
			}
		},
	};
}
