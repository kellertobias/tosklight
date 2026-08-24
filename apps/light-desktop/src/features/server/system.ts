import { saveServerUrl } from "../../api/client/serverLocation";
import type { CueList, StoredGroup, StoredPreset } from "../../api/types";
import type { ServerController } from "./model";
import type { ServerCapabilities } from "./capabilityContracts";

/**
 * Paperwork is a one-shot operator export, so it reads the current Patch on demand instead of
 * keeping a broad Patch snapshot resident. A Patch read failure degrades the export rather than
 * blocking it or masking a more relevant collection error.
 */
async function readPatchForPaperwork(api: ServerController["api"]) {
	try {
		return await api.fixtures.patch();
	} catch {
		return null;
	}
}

export function createSystemActions(
	model: ServerController,
): Pick<
	ServerCapabilities,
	| "exportPaperwork"
	| "shutdownServer"
	| "clearProgrammer"
	| "setDeskToken"
	| "setServerUrl"
> {
	const {
		api,
		setError,
		bootstrap,
		session,
		commandTargetModeRef,
		setCommandLineState,
		setCommandLinePristine,
		setSelectedFixtures,
		setSelectedGroupId,
	} = model;
	return {
		exportPaperwork: async () => {
			try {
				const showId = bootstrap?.active_show?.id;
				const patch = showId ? await readPatchForPaperwork(api) : null;
				const [groups, presets, cueLists] = showId
					? await Promise.all([
							api.showObjects.objects<StoredGroup>(showId, "group"),
							api.showObjects.objects<StoredPreset>(showId, "preset"),
							api.showObjects.objects<CueList>(showId, "cue_list"),
						])
					: [[], [], []];
				const payload = {
					generated_at: new Date().toISOString(),
					show: bootstrap?.active_show,
					patch,
					cue_lists: cueLists.map((item) => item.body),
					groups: groups.map((item) => item.body),
					presets: presets.map((item) => ({
						id: item.id,
						name: item.body.name,
						fixtures: Object.keys(item.body.values).length,
					})),
				};
				const blob = new Blob([JSON.stringify(payload, null, 2)], {
					type: "application/json",
				});
				const url = URL.createObjectURL(blob);
				const anchor = document.createElement("a");
				anchor.href = url;
				anchor.download = `${bootstrap?.active_show?.name ?? "show"}-paperwork.json`;
				anchor.click();
				URL.revokeObjectURL(url);
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		shutdownServer: async () => {
			try {
				await api.desk.shutdown();
				setError(null);
				return true;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				return false;
			}
		},
		clearProgrammer: async (sessionId) => {
			try {
				await api.desk.clearProgrammer(sessionId);
				if (sessionId === session?.session_id) {
					setSelectedFixtures([]);
					setSelectedGroupId(null);
					setCommandLineState(commandTargetModeRef.current);
					setCommandLinePristine(true);
				}
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		setDeskToken: (token) => {
			api.runtime.setDeskToken(token);
			location.reload();
		},
		setServerUrl: (url) => {
			try {
				saveServerUrl(url);
				location.reload();
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
	};
}
