import type { OutputRoute } from "../../api/types";
import type { ServerController } from "./model";
import type { ServerCapabilities } from "./capabilityContracts";

export function createOutputActions(
	model: ServerController,
): Pick<
	ServerCapabilities,
	| "readDmx"
	| "readVisualization"
	| "setDmxOverride"
	| "saveOutputRoute"
	| "deleteOutputRoute"
> {
	const { api, setError, bootstrap, setOutputRoutes } = model;
	return {
		readDmx: () => api.mediaOutput.dmx(),
		readVisualization: (preload = false) => api.mediaOutput.visualization(preload),
		setDmxOverride: async (universe, address, rawValue) => {
			try {
				await api.mediaOutput.setDmxOverride(universe, address, rawValue);
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		saveOutputRoute: async (id, route, revision) => {
			if (!bootstrap?.active_show) return false;
			try {
				await api.showObjects.saveOutputRoute(
					bootstrap.active_show.id,
					id,
					route,
					revision,
				);
				setOutputRoutes(
					await api.showObjects.objects<OutputRoute>(bootstrap.active_show.id, "route"),
				);
				setError(null);
				return true;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				return false;
			}
		},
		deleteOutputRoute: async (id, revision) => {
			if (!bootstrap?.active_show) return false;
			try {
				await api.showObjects.deleteOutputRoute(
					bootstrap.active_show.id,
					id,
					revision,
				);
				setOutputRoutes(
					await api.showObjects.objects<OutputRoute>(bootstrap.active_show.id, "route"),
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
