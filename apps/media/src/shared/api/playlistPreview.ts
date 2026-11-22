/** Typed live preview intents; no configuration write and no reconnect replay. */
import { request } from "./client";
import type { PreviewPlaylistStep, StopPlaylistPreview, PlaylistPreviewResult } from "./generated/media-wire";
import { KEYS } from "./queries";
import { writeResource } from "./resource";
import type { OutputView } from "./generated/media-wire";
import { sendTelemetryActionResult } from "./telemetry";

export async function showPlaylistStep(request: PreviewPlaylistStep): Promise<PlaylistPreviewResult> {
	const result = await sendTelemetryActionResult({ type: "automation-preview-step", request });
	if (!result.preview) throw new Error("The server did not confirm the playlist preview");
	return result.preview;
}
export async function stopPlaylistPreview(request: StopPlaylistPreview): Promise<PlaylistPreviewResult> {
	const result = await sendTelemetryActionResult({ type: "automation-stop-preview", request });
	if (!result.preview) throw new Error("The server did not confirm that preview stopped");
	return result.preview;
}
/** Cleanup is a separate, session-scoped Stop; it can never start or replay playback. */
export function cleanupPlaylistPreview(body: StopPlaylistPreview): Promise<PlaylistPreviewResult> {
	return request("/automation/preview-stop", { method: "POST", body: JSON.stringify(body), keepalive: true });
}
export function acceptPreviewOutput(result: PlaylistPreviewResult, current: OutputView[] | undefined) {
	writeResource(KEYS.outputs, (current ?? []).some(output => output.id === result.output.id)
		? current!.map(output => output.id === result.output.id ? result.output : output)
		: [...(current ?? []), result.output]);
}
