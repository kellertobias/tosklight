import { saveServerUrl } from "../../api/LightApiClient";
import type { StoredGroup, StoredPreset } from "../../api/types";
import type { ServerController } from "./model";
import type { ServerContextValue } from "./ServerContextValue";

export function createSystemActions(
	model: ServerController,
): Pick<
	ServerContextValue,
	| "switchUser"
	| "exportPaperwork"
	| "shutdownServer"
	| "clearProgrammer"
	| "setMaster"
	| "setDeskToken"
	| "setServerUrl"
> {
	const {
		client,
		setError,
		bootstrap,
		setBootstrap,
		session,
		patch,
		playbacks,
		commandTargetModeRef,
		setCommandLineState,
		setCommandLinePristine,
		setSelectedFixtures,
		setSelectedGroupId,
	} = model;
	return {
		switchUser: (name) => {
			localStorage.setItem("light.operator", name);
			location.reload();
		},
		exportPaperwork: async () => {
			try {
				const showId = bootstrap?.active_show?.id;
				const [groups, presets] = showId
					? await Promise.all([
							client.objects<StoredGroup>(showId, "group"),
							client.objects<StoredPreset>(showId, "preset"),
						])
					: [[], []];
				const payload = {
					generated_at: new Date().toISOString(),
					show: bootstrap?.active_show,
					patch,
					cue_lists: playbacks?.cue_lists,
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
				await client.shutdown();
				setError(null);
				return true;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				return false;
			}
		},
		clearProgrammer: async (sessionId) => {
			try {
				await client.clearProgrammer(sessionId);
				if (sessionId === session?.session_id) {
					setSelectedFixtures([]);
					setSelectedGroupId(null);
					setCommandLineState(commandTargetModeRef.current);
					setCommandLinePristine(true);
				}
				setBootstrap(await client.bootstrap());
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		setMaster: async (grandMaster, blackout) => {
			try {
				await client.setMaster({ grand_master: grandMaster, blackout });
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		setDeskToken: (token) => {
			client.setDeskToken(token);
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
