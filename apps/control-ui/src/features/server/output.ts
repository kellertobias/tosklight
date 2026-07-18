import type { OutputRoute } from "../../api/types";
import type { ServerController } from "./model";
import type { ServerContextValue } from "./ServerContextValue";

export function createOutputActions(
	model: ServerController,
): Pick<
	ServerContextValue,
	| "readDmx"
	| "readVisualization"
	| "setDmxOverride"
	| "saveOutputRoute"
	| "deleteOutputRoute"
> {
	const { client, setError, bootstrap, setPatch, setOutputRoutes } = model;
	return {
		readDmx: () => client.dmx(),
		readVisualization: (preload = false) => client.visualization(preload),
		setDmxOverride: async (universe, address, rawValue) => {
			try {
				await client.setDmxOverride(universe, address, rawValue);
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		saveOutputRoute: async (id, route, revision) => {
			if (!bootstrap?.active_show) return false;
			try {
				await client.putObject(
					bootstrap.active_show.id,
					"route",
					id,
					route,
					revision,
				);
				setPatch(await client.patch());
				setOutputRoutes(
					await client.objects<OutputRoute>(bootstrap.active_show.id, "route"),
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
				await client.deleteObject(
					bootstrap.active_show.id,
					"route",
					id,
					revision,
				);
				setPatch(await client.patch());
				setOutputRoutes(
					await client.objects<OutputRoute>(bootstrap.active_show.id, "route"),
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
