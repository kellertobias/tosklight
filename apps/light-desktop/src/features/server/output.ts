import { formatErrorDetails } from "@tosklight/ui";
import type { OutputRoute } from "../../api/types";
import type { ServerCapabilities } from "./capabilityContracts";
import type { ServerController } from "./model";

export function createOutputActions(
	model: ServerController,
): Pick<
	ServerCapabilities,
	| "readDmx"
	| "readOutputHealth"
	| "resetChangeLeadTime"
	| "readNetworkEndpoints"
	| "readVisualization"
	| "setDmxOverride"
	| "saveOutputRoute"
	| "createOutputRouteRange"
	| "deleteOutputRoute"
> {
	const { api, setError, bootstrap, setOutputRoutes } = model;
	return {
		readDmx: () => api.mediaOutput.dmx(),
		readOutputHealth: () => api.runtime.outputHealth(),
		resetChangeLeadTime: () => api.mediaOutput.resetChangeLeadTime(),
		readNetworkEndpoints: () => api.mediaOutput.networkEndpoints(),
		readVisualization: (preload = false) =>
			api.mediaOutput.visualization(preload),
		setDmxOverride: async (universe, address, rawValue) => {
			try {
				await api.mediaOutput.setDmxOverride(universe, address, rawValue);
				setError(null);
			} catch (reason) {
				setError(formatErrorDetails(reason));
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
					await api.showObjects.objects<OutputRoute>(
						bootstrap.active_show.id,
						"route",
					),
				);
				setError(null);
				return true;
			} catch (reason) {
				setError(formatErrorDetails(reason));
				return false;
			}
		},
		createOutputRouteRange: async (range) => {
			if (!bootstrap?.active_show) return false;
			try {
				await api.showObjects.createOutputRouteRange(
					bootstrap.active_show.id,
					range,
				);
				setOutputRoutes(
					await api.showObjects.objects<OutputRoute>(
						bootstrap.active_show.id,
						"route",
					),
				);
				setError(null);
				return true;
			} catch (reason) {
				setError(formatErrorDetails(reason));
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
					await api.showObjects.objects<OutputRoute>(
						bootstrap.active_show.id,
						"route",
					),
				);
				setError(null);
				return true;
			} catch (reason) {
				setError(formatErrorDetails(reason));
				return false;
			}
		},
	};
}
