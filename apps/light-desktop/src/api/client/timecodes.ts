import type {
	ShowObjectActionOutcome,
	TimecodeCollectionSnapshot as WireTimecodeCollectionSnapshot,
	TimecodeDefinition as WireTimecodeDefinition,
	TimecodeObjectAction as WireTimecodeObjectAction,
	TimecodeTransportSnapshot as WireTimecodeTransportSnapshot,
} from "../generated/light-wire";
import type {
	TimecodeAudioImportResult,
	TimecodeAudioOutputDevices,
	TimecodeAudioWaveform,
	TimecodeCollectionSnapshot,
	TimecodeDefinition,
	TimecodeObjectAction,
	TimecodeObjectRecord,
	TimecodePatch,
	TimecodeTransportAction,
	TimecodeTransportSnapshot,
} from "../types/timecode";
import { jsonRequest, type LiveClientTransport } from "./transport";

export type {
	TimecodeAudioImportResult,
	TimecodeAudioOutputDevices,
	TimecodeAudioWaveform,
	TimecodeCollectionSnapshot,
	TimecodeDefinition,
	TimecodeObjectAction,
	TimecodeObjectRecord,
	TimecodePatch,
	TimecodeTransportAction,
	TimecodeTransportSnapshot,
} from "../types/timecode";

export class TimecodesApiClient {
	constructor(private readonly transport: LiveClientTransport) {}

	async objects(showId: string): Promise<TimecodeCollectionSnapshot> {
		const snapshot =
			await this.transport.request<WireTimecodeCollectionSnapshot>(
				"/api/v2/timecodes",
				{
					headers: showHeaders(showId),
				},
			);
		return {
			show_revision: snapshot.show_revision,
			objects: snapshot.objects.map(timecodeObject),
		};
	}

	mutate(showId: string, action: TimecodeObjectAction) {
		return this.post<ShowObjectActionOutcome>(
			"/api/v2/timecodes/actions",
			showId,
			{
				request_id: crypto.randomUUID(),
				action: wireAction(action),
			},
		);
	}

	create(showId: string, definition: TimecodeDefinition) {
		return this.mutate(showId, { type: "create", definition });
	}

	update(
		showId: string,
		timecodeId: string,
		expectedRevision: number,
		patch: TimecodePatch,
	) {
		return this.mutate(showId, {
			type: "update",
			timecode_id: timecodeId,
			expected_revision: expectedRevision,
			patch,
		});
	}

	delete(showId: string, timecodeId: string, expectedRevision: number) {
		return this.mutate(showId, {
			type: "delete",
			timecode_id: timecodeId,
			expected_revision: expectedRevision,
		});
	}

	async runtime(showId: string): Promise<TimecodeTransportSnapshot[]> {
		const snapshots = await this.transport.request<
			WireTimecodeTransportSnapshot[]
		>("/api/v2/timecodes/runtime", {
			headers: showHeaders(showId),
		});
		return snapshots.map(timecodeSnapshot);
	}

	snapshot(
		showId: string,
		timecodeId: string,
	): Promise<TimecodeTransportSnapshot> {
		return this.transport
			.request<WireTimecodeTransportSnapshot>(
				`/api/v2/timecodes/${encodeURIComponent(timecodeId)}/runtime`,
				{
					headers: showHeaders(showId),
				},
			)
			.then(timecodeSnapshot);
	}

	async transportAction(
		_showId: string,
		timecodeId: string,
		action: TimecodeTransportAction,
	) {
		const value = await this.transport.sendAction({
			type: "timecode",
			request: { timecode_id: timecodeId, action },
		});
		return timecodeSnapshot(value as WireTimecodeTransportSnapshot);
	}

	outputDevices(): Promise<TimecodeAudioOutputDevices> {
		return this.transport.request("/api/v2/timecodes/audio/outputs");
	}

	importAudio(showId: string, file: File): Promise<TimecodeAudioImportResult> {
		return this.transport.request(
			`/api/v2/timecodes/audio/import?name=${encodeURIComponent(file.name)}`,
			{
				method: "POST",
				headers: {
					"content-type": file.type || mediaTypeForName(file.name),
					...showHeaders(showId),
				},
				body: file,
			},
		);
	}

	waveform(showId: string, timecodeId: string): Promise<TimecodeAudioWaveform> {
		return this.transport.request(
			`/api/v2/timecodes/${encodeURIComponent(timecodeId)}/audio/waveform`,
			{ headers: showHeaders(showId) },
		);
	}

	private post<T>(path: string, showId: string, body: unknown): Promise<T> {
		const request = jsonRequest("POST", body);
		return this.transport.request(path, {
			...request,
			headers: { ...request.headers, ...showHeaders(showId) },
		});
	}
}

function timecodeObject(object: {
	revision: number;
	definition: WireTimecodeDefinition;
}): TimecodeObjectRecord {
	return {
		revision: object.revision,
		definition: timecodeDefinition(object.definition),
	};
}

function timecodeDefinition(
	definition: WireTimecodeDefinition,
): TimecodeDefinition {
	return {
		...definition,
		audio: definition.audio ? { ...definition.audio } : definition.audio,
		markers: definition.markers.map((marker) => ({ ...marker })),
		lanes: definition.lanes.map((lane) => ({
			...lane,
			content: cloneLaneContent(lane.content),
		})),
	};
}

function cloneLaneContent(
	content: WireTimecodeDefinition["lanes"][number]["content"],
): TimecodeDefinition["lanes"][number]["content"] {
	if (content.kind === "cue_list") {
		return { ...content, clips: content.clips.map((clip) => ({ ...clip })) };
	}
	if (content.kind === "speed_group") {
		return {
			...content,
			keyframes: content.keyframes.map((keyframe) => ({ ...keyframe })),
		};
	}
	return {
		...content,
		keyframes: content.keyframes.map((keyframe) => ({ ...keyframe })),
	};
}

function wireAction(action: TimecodeObjectAction): WireTimecodeObjectAction {
	if (action.type === "create") {
		return { type: "create", definition: action.definition };
	}
	return { ...action };
}

function timecodeSnapshot(
	snapshot: WireTimecodeTransportSnapshot,
): TimecodeTransportSnapshot {
	return { ...snapshot };
}

function showHeaders(showId: string): HeadersInit {
	return { "x-tosk-show": showId };
}

function mediaTypeForName(name: string): string {
	return name.toLowerCase().endsWith(".mp3") ? "audio/mpeg" : "audio/wav";
}
