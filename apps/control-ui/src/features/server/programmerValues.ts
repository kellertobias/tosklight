import type { ServerController } from "./model";
import type { ServerContextValue } from "./ServerContextValue";

export function createProgrammerValueActions(
	model: ServerController,
): Pick<
	ServerContextValue,
	| "setProgrammer"
	| "setProgrammerMany"
	| "setProgrammerValue"
	| "controlFixtureAction"
	| "generateFixturePresets"
	| "releaseProgrammer"
	| "setGroupValue"
	| "releaseGroupValue"
	| "setPreloadGroupValue"
> {
	const {
		client,
		setError,
		bootstrap,
		setBootstrap,
		selectedGroupId,
	} = model;
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
		setProgrammerValue: async (fixtureId, attribute, value) => {
			try {
				await client.setProgrammerValue(fixtureId, attribute, value);
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
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
		releaseProgrammer: async (fixtureId, attribute) => {
			try {
				await client.releaseProgrammer(fixtureId, attribute);
				setBootstrap(await client.bootstrap());
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		setGroupValue: async (attribute, level) => {
			try {
				if (!selectedGroupId)
					throw new Error(
						"Select a live group before setting group-relative values",
					);
				await client.setGroupProgrammer(selectedGroupId, attribute, level);
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		releaseGroupValue: async (attribute) => {
			try {
				if (!selectedGroupId)
					throw new Error(
						"Select a live group before releasing group-relative values",
					);
				await client.releaseGroupProgrammer(selectedGroupId, attribute);
				setBootstrap(await client.bootstrap());
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		setPreloadGroupValue: async (attribute, level) => {
			try {
				if (!selectedGroupId)
					throw new Error(
						"Select a live group before setting group-relative preload values",
					);
				await client.setPreloadGroup(selectedGroupId, attribute, level);
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
	};
}
