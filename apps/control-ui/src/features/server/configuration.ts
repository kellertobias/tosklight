import type { ServerController } from "./model";
import type { ServerCapabilities } from "./capabilityContracts";

export function createConfigurationActions(
	model: ServerController,
): Pick<
	ServerCapabilities,
	| "speedGroup"
	| "updateSpeedGroup"
	| "observeSpeedGroup"
	| "speedGroupAction"
> {
	const { api, setError } = model;
	return {
		speedGroup: (group) => api.desk.speedGroup(group),
		updateSpeedGroup: async (group, next) => {
			try {
				const result = await api.desk.updateSpeedGroup(group, next);
				setError(null);
				return result;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				throw reason;
			}
		},
		observeSpeedGroup: (group, observation) =>
			api.desk.observeSpeedGroup(group, observation),
		speedGroupAction: async (group, input) => {
			try {
				const result = await api.desk.speedGroupAction(group, input);
				setError(null);
				return result;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				throw reason;
			}
		},
	};
}
