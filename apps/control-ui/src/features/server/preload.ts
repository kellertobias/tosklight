import type { ServerController } from "./model";
import type { ServerContextValue } from "./ServerContextValue";

export function createPreloadActions(
	model: ServerController,
): Pick<ServerContextValue, "preloadAction" | "storePreload" | "storeDynamic"> {
	const {
		client,
		setError,
		bootstrap,
		cueObjects,
		selectedFixtures,
		selectedGroupId,
		refresh,
	} = model;
	return {
		preloadAction: async (action) => {
			try {
				await client.preload(action);
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		storePreload: async (input, revision) => {
			try {
				if (!bootstrap?.active_show)
					throw new Error("Open a show before storing preload data");
				await client.storePreload(bootstrap.active_show.id, input, revision);
				await refresh();
				setError(null);
				return true;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				return false;
			}
		},
		storeDynamic: async (speed, width, direction) => {
			try {
				if (!bootstrap?.active_show)
					throw new Error("Open a show before storing a dynamic");
				const target = cueObjects[0];
				if (!target)
					throw new Error("Create a Cuelist before storing a dynamic");
				const body = structuredClone(target.body) as {
					cues?: Array<{ phasers?: unknown[] }>;
				};
				const cue = body.cues?.[0];
				if (!cue) throw new Error("The Cuelist needs at least one Cue");
				const phasers = cue.phasers ?? [];
				cue.phasers = phasers;
				phasers.push({
					fixture_ids: selectedGroupId ? [] : selectedFixtures,
					group_ids: selectedGroupId ? [selectedGroupId] : [],
					attribute: "intensity",
					phaser: {
						mode: "relative",
						steps: [
							{ position: 0, value: 0, curve_to_next: "sine" },
							{ position: 0.5, value: 1, curve_to_next: "sine" },
						],
						cycles_per_minute: speed,
						phase_start_degrees: direction === "Reverse" ? 360 : 0,
						phase_end_degrees: direction === "Reverse" ? 0 : 360,
						width: width / 100,
					},
				});
				await client.putObject(
					bootstrap.active_show.id,
					"cue_list",
					target.id,
					body,
					target.revision,
				);
				await refresh();
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
	};
}
