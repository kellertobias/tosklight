import type { ServerController } from "./model";
import type { ServerContextValue } from "./ServerContextValue";

export function createUpdateActions(
	model: ServerController,
): Pick<
	ServerContextValue,
	| "updateSettings"
	| "saveUpdateSettings"
	| "previewUpdate"
	| "applyUpdate"
	| "updateTargets"
> {
	const { client, setError, refresh } = model;
	return {
		updateSettings: async () => {
			try {
				const settings = await client.updateSettings();
				setError(null);
				return settings;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				return null;
			}
		},
		saveUpdateSettings: async (settings) => {
			try {
				await client.saveUpdateSettings(settings);
				setError(null);
				return true;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				return false;
			}
		},
		previewUpdate: async (target, mode) => {
			try {
				const preview = await client.previewUpdate(target, mode);
				setError(null);
				return preview;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				return null;
			}
		},
		applyUpdate: async (
			target,
			mode,
			expectedRevision,
			expectedProgrammerRevision,
			expectedShowRevision,
		) => {
			try {
				const result = await client.applyUpdate(
					target,
					mode,
					expectedRevision,
					expectedProgrammerRevision,
					expectedShowRevision,
				);
				await refresh();
				setError(null);
				return result;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				return null;
			}
		},
		updateTargets: async (filter) => {
			try {
				const entries = await client.updateTargets(filter);
				setError(null);
				return entries;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				return null;
			}
		},
	};
}
