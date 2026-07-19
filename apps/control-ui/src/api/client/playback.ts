import type {
	ControlDesk,
	PlaybackDefinition,
	PlaybackPage,
	PlaybackSnapshot,
	ScreenConfiguration,
	ScreenSnapshot,
	VirtualPlaybackExclusionSnapshot,
	VirtualPlaybackExclusionZone,
} from "../types";
import type { LiveClientTransport } from "./transport";

type PoolPlaybackAction =
	| "button"
	| "on"
	| "off"
	| "toggle"
	| "go"
	| "go-minus"
	| "go-to"
	| "load"
	| "fast-forward"
	| "fast-rewind"
	| "temp"
	| "temp-on"
	| "temp-off"
	| "swap"
	| "select"
	| "select-contents"
	| "select-dereferenced"
	| "learn"
	| "double"
	| "half"
	| "pause"
	| "blackout"
	| "pause-dynamics"
	| "flash"
	| "master"
	| "xfade-on"
	| "xfade-off";

interface PoolPlaybackInput {
	value?: number;
	pressed?: boolean;
	button?: number;
	cue_number?: number;
	surface?: "physical" | "virtual";
}

export class PlaybackApiClient {
	constructor(private readonly transport: LiveClientTransport) {}

	playbacks(): Promise<PlaybackSnapshot> {
		return this.transport.request("/api/v1/playbacks");
	}

	screens(): Promise<ScreenSnapshot> {
		return this.transport.request("/api/v1/screens");
	}

	putScreen(screen: ScreenConfiguration): Promise<ScreenConfiguration> {
		return this.transport.request(`/api/v1/screens/${screen.id}`, {
			method: "PUT",
			headers: { "content-type": "application/json" },
			body: JSON.stringify(screen),
		});
	}

	deleteScreen(id: string): Promise<void> {
		return this.transport.request(`/api/v1/screens/${id}`, {
			method: "DELETE",
		});
	}

	setScreenPage(id: string, page: number) {
		return this.transport.request(`/api/v1/screens/${id}/page`, {
			method: "PUT",
			headers: { "content-type": "application/json" },
			body: JSON.stringify({ page }),
		});
	}

	playbackAction(
		cueListId: string,
		action: "go" | "back" | "pause" | "release",
	) {
		return this.transport.command(`playback.${action}`, {
			cue_list_id: cueListId,
		});
	}

	poolPlaybackAction(
		number: number,
		action: PoolPlaybackAction,
		input: PoolPlaybackInput = {},
	) {
		return this.transport.request(`/api/v1/cuelists/${number}/${action}`, {
			method: action === "master" ? "PUT" : "POST",
			headers: { "content-type": "application/json" },
			body: JSON.stringify(input),
		});
	}

	virtualPlaybackExclusionZones(): Promise<VirtualPlaybackExclusionSnapshot> {
		return this.transport.request("/api/v1/virtual-playback-exclusion-zones");
	}

	saveVirtualPlaybackExclusionZones(
		surfaceId: string,
		zones: VirtualPlaybackExclusionZone[],
	) {
		return this.transport.request<{
			surface_id: string;
			zones: VirtualPlaybackExclusionZone[];
		}>(
			`/api/v1/virtual-playback-exclusion-zones/${encodeURIComponent(surfaceId)}`,
			{
				method: "PUT",
				headers: { "content-type": "application/json" },
				body: JSON.stringify({ zones }),
			},
		);
	}

	savePlaybackSlot(
		page: number,
		slot: number,
		playback: PlaybackDefinition,
		expectedPlaybackRevision: number,
		expectedPageRevision: number,
	) {
		return this.transport.request<{
			playback: PlaybackDefinition;
			page: PlaybackPage;
		}>(`/api/v1/playback-pages/${page}/slots/${slot}`, {
			method: "PUT",
			headers: { "content-type": "application/json" },
			body: JSON.stringify({
				playback,
				expected_playback_revision: expectedPlaybackRevision,
				expected_page_revision: expectedPageRevision,
			}),
		});
	}

	clearPlaybackSlot(
		page: number,
		slot: number,
		expectedPlaybackRevision: number,
		expectedPageRevision: number,
	) {
		return this.transport.request<{
			cleared: boolean;
			playback_number: number;
			page: number;
			slot: number;
			page_revisions: number[];
		}>(`/api/v1/playback-pages/${page}/slots/${slot}`, {
			method: "DELETE",
			headers: { "content-type": "application/json" },
			body: JSON.stringify({
				expected_playback_revision: expectedPlaybackRevision,
				expected_page_revision: expectedPageRevision,
			}),
		});
	}

	setPlaybackPage(deskId: string, page: number) {
		return this.transport.request(`/api/v1/control-desks/${deskId}/page`, {
			method: "PUT",
			headers: { "content-type": "application/json" },
			body: JSON.stringify({ page }),
		});
	}

	updateControlDesk(desk: ControlDesk): Promise<ControlDesk> {
		return this.transport.request(`/api/v1/control-desks/${desk.id}`, {
			method: "PUT",
			headers: { "content-type": "application/json" },
			body: JSON.stringify(desk),
		});
	}

	removeClient(deskId: string): Promise<void> {
		return this.transport.request(`/api/v1/clients/${deskId}`, {
			method: "DELETE",
		});
	}
}
