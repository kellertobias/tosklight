import type { ServerController } from "./model";
import type { ServerContextValue } from "./ServerContextValue";

export function createConfigurationActions(
	model: ServerController,
): Pick<
	ServerContextValue,
	| "saveConfiguration"
	| "setControlTiming"
	| "speedGroup"
	| "updateSpeedGroup"
	| "observeSpeedGroup"
	| "speedGroupAction"
> {
	const { client, setError, configuration, setConfiguration, setMatter } =
		model;
	return {
		saveConfiguration: async (next) => {
			try {
				const result = await client.updateConfiguration(next);
				setConfiguration(result.configuration);
				setMatter(result.matter);
				setError(null);
				return result.requires_restart;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				return false;
			}
		},
		setControlTiming: async (input) => {
			if (!configuration) return;
			try {
				const result = await client.updateConfiguration({
					...configuration,
					...input,
				});
				setConfiguration(result.configuration);
				setMatter(result.matter);
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
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
