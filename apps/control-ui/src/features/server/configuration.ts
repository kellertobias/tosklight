import type { ServerController } from "./model";
import type { ServerContextValue } from "./ServerContextValue";

export function createConfigurationActions(
	model: ServerController,
): Pick<
	ServerContextValue,
	| "speedGroup"
	| "updateSpeedGroup"
	| "observeSpeedGroup"
	| "speedGroupAction"
> {
	const { client, setError } = model;
	return {
		speedGroup: (group) => client.speedGroup(group),
		updateSpeedGroup: async (group, next) => {
			try {
				const result = await client.updateSpeedGroup(group, next);
				setError(null);
				return result;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				throw reason;
			}
		},
		observeSpeedGroup: (group, observation) =>
			client.observeSpeedGroup(group, observation),
		speedGroupAction: async (group, input) => {
			try {
				const result = await client.speedGroupAction(group, input);
				setError(null);
				return result;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				throw reason;
			}
		},
	};
}
