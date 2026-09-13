import type { MediaServerFixture } from "../../api/types";
import type { BuildMediaPaneModelInput } from "./buildMediaPaneModel";
import type { MediaPaneLayer, MediaPreviewState } from "./mediaPaneModel";

export function serverChoices(input: BuildMediaPaneModelInput) {
	if (input.servers.length === 0 && !input.selectedServerId)
		return [
			{
				id: "",
				name: "No media server is patched",
				statusLabel: "Missing patch",
				disabled: true,
			},
		];
	return [
		...(input.selectedServerId && !input.selectedServer
			? [
					{
						id: input.selectedServerId,
						name: "Configured server unavailable",
						statusLabel: "Missing patch",
						disabled: true,
					},
				]
			: []),
		...input.servers.map((server) => ({
			id: server.fixture_id,
			name: server.name,
			fixtureLabel:
				server.fixture_number == null
					? server.fixture_id
					: String(server.fixture_number),
			statusLabel: isAudioPlayer(server)
				? server.status.online
					? "Internal"
					: "Unavailable"
				: !server.endpoint
					? "Not configured"
					: server.status.online
						? "Online"
						: "Offline",
		})),
	];
}

/**
 * Condenses a media-server failure into something an operator can act on.
 *
 * The raw CITP text ("CITP I/O error: Connection refused (os error 61)") says nothing useful on a
 * dark stage, so the pane headline states the situation and the protocol text stays available as
 * the element's title and in Show Patch > Media Servers.
 */
export function mediaOfflineReason(diagnostic: string | null): string {
	if (!diagnostic) return "Not responding";
	if (/timed out/iu.test(diagnostic)) return "Not responding";
	if (
		/refused|unreachable|no route|reset|broken pipe|closed/iu.test(diagnostic)
	)
		return "Connection refused";
	if (/invalid CITP packet/iu.test(diagnostic)) return "Unexpected reply";
	if (/rejected/iu.test(diagnostic)) return "Request rejected";
	return "Not responding";
}

export function previewState(
	input: BuildMediaPaneModelInput,
): MediaPreviewState {
	if (!input.selectedServer)
		return {
			kind: "missing_patch",
			detail: "No media server is patched.",
		};
	if (isAudioPlayer(input.selectedServer))
		return {
			kind: "audio",
			detail:
				input.selectedServer.status.last_error ??
				audioSourceLabel(input.selectedServer),
		};
	if (!input.selectedServer.endpoint)
		return {
			kind: "offline",
			detail:
				"No CITP Media Server is available. Configure one in Show Patch > Media Servers.",
		};
	if (input.inspectionError || !input.selectedServer.status.online) {
		const diagnostic =
			input.inspectionError ?? input.selectedServer.status.last_error ?? null;
		return {
			kind: "offline",
			detail: mediaOfflineReason(diagnostic),
			...(diagnostic ? { diagnostic } : {}),
		};
	}
	const source = input.inspection.preview_sources.find(
		(candidate) => candidate.layer == null,
	);
	return source
		? {
				kind: "ready",
				outputSize: { width: source.width, height: source.height },
				imageSrc:
					input.previewUrls[`${input.selectedServer.fixture_id}:${source.id}`],
			}
		: {
				kind: "unsupported",
				capability: "preview",
				detail: "No composite preview source is advertised.",
			};
}

/** The desk-local Internal Audio Player has no CITP endpoint and no advertised library. */
export function isAudioPlayer(server: MediaServerFixture | undefined) {
	return server?.kind === "audio_player";
}

function audioSourceLabel(server: MediaServerFixture) {
	const folder = server.audio?.folder ?? 0;
	const file = server.audio?.file ?? 0;
	if (!folder || !file) return "No audio source selected";
	return server.audio?.source ?? `${pad(folder)} / ${pad(file)}`;
}

function pad(value: number) {
	return String(value).padStart(3, "0");
}

function audioLayerModels(server: MediaServerFixture): MediaPaneLayer[] {
	const audio = server.audio;
	const failed = Boolean(server.status.last_error);
	return server.layers.map((head, index) => ({
		id: head.fixture_id,
		number: String(index + 1),
		name: "Player",
		status: failed ? "failed" : "online",
		statusLabel: failed
			? "Failed"
			: audio?.transport === "play"
				? "Playing"
				: audio?.transport === "pause"
					? "Paused"
					: "Stopped",
		errorDetail: server.status.last_error ?? undefined,
		audio: {
			volumeLabel: `${audio?.volume_percent ?? 0}%`,
			sourceLabel: `${pad(audio?.folder ?? 0)} / ${pad(audio?.file ?? 0)}`,
		},
		liveSourceLabel: audioSourceLabel(server),
	}));
}

export function layerModels(input: BuildMediaPaneModelInput): MediaPaneLayer[] {
	if (isAudioPlayer(input.selectedServer) && input.selectedServer)
		return audioLayerModels(input.selectedServer);
	return (input.selectedServer?.layers ?? []).map((head, citpLayer) => {
		const status = input.inspection.layers.find(
			(layer) => layer.layer === citpLayer,
		);
		const source = input.inspection.preview_sources.find(
			(candidate) => candidate.layer === citpLayer,
		);
		return {
			id: head.fixture_id,
			number: String(citpLayer + 1),
			name: status?.name || `Layer ${citpLayer + 1}`,
			status:
				status?.flags && status.flags & 0x8
					? "failed"
					: status
						? "online"
						: "unsupported",
			statusLabel: status
				? status.flags & 0x4
					? "Loading"
					: status.flags & 0x8
						? "Failed"
						: "Online"
				: "No advertised mapping",
			errorDetail:
				status?.flags && status.flags & 0x8
					? "The Media Server could not render this layer. Check its Media Server logs."
					: undefined,
			thumbnailSrc:
				source && status && (status.folder !== 0 || status.file !== 0)
					? input.previewUrls[
							`${input.selectedServer?.fixture_id}:${source.id}`
						]
					: undefined,
			liveSourceLabel: status
				? `Folder ${status.folder} · File ${status.file}`
				: undefined,
		};
	});
}
