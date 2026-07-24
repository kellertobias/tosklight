import type {
	ControlDeskConfigurationAction,
	ControlDeskConfigurationActionOutcome,
	ControlDeskConfigurationActionRequest,
	ControlDeskConfigurationPatch,
	PlaybackActionOutcome,
	PlaybackActionRequest,
	PlaybackRuntimeIdentity,
	PlaybackRuntimeSnapshot,
	ScreenConfigurationAction,
	ScreenConfigurationActionOutcome,
	ScreenConfigurationActionRequest,
	ScreenConfigurationPatch,
	ScreenConfiguration as WireScreenConfiguration,
} from "../generated/light-wire";
import { decodePlaybackOutcome, decodePlaybackSnapshot } from "../playbackWire";
import type {
	ControlDesk,
	ScreenConfiguration,
	ScreenSnapshot,
} from "../types";
import { WireValidationError } from "../wireValidation";
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

interface PlaybackPageSelectionOptions {
	existingOnly?: boolean;
}

export class PlaybackApiClient {
	private screensById = new Map<string, ScreenConfiguration>();
	private screensLoaded = false;

	constructor(private readonly transport: LiveClientTransport) {}

	async playbackRuntimeSnapshot(
		deskId: string,
		identities: PlaybackRuntimeIdentity[],
	): Promise<PlaybackRuntimeSnapshot> {
		const value = await this.transport.request<unknown>(
			"/api/v2/playback-runtime/snapshot",
			{
				method: "POST",
				headers: {
					"content-type": "application/json",
					"x-tosk-desk": deskId,
				},
				body: JSON.stringify({ identities }),
			},
		);
		return decodePlaybackSnapshot(value);
	}

	async playbackRuntimeAction(
		showId: string,
		deskId: string,
		request: PlaybackActionRequest,
	): Promise<PlaybackActionOutcome> {
		const value = await this.transport.request<unknown>(
			"/api/v2/playback-actions",
			{
				method: "POST",
				headers: {
					"content-type": "application/json",
					"x-tosk-show": showId,
					"x-tosk-desk": deskId,
				},
				body: JSON.stringify(request),
			},
		);
		const outcome = decodePlaybackOutcome(value);
		if (outcome.request_id !== request.request_id)
			throw new WireValidationError(
				"$.request_id",
				`request ID ${request.request_id}`,
				outcome.request_id,
			);
		return outcome;
	}

	async playbackRuntimeLiveAction(
		request: PlaybackActionRequest,
	): Promise<PlaybackActionOutcome> {
		const value = await this.transport.commandWithRequestId(
			"playback.action",
			request,
			request.request_id,
		);
		const outcome = decodePlaybackOutcome(value);
		if (outcome.request_id !== request.request_id)
			throw new WireValidationError(
				"$.request_id",
				`request ID ${request.request_id}`,
				outcome.request_id,
			);
		return outcome;
	}

	async screens(): Promise<ScreenSnapshot> {
		const snapshot =
			await this.transport.request<ScreenSnapshot>("/api/v2/screens");
		this.screensById = new Map(
			snapshot.screens.map((screen) => [screen.id, screen]),
		);
		this.screensLoaded = true;
		return snapshot;
	}

	async putScreen(screen: ScreenConfiguration): Promise<ScreenConfiguration> {
		if (!this.screensLoaded) await this.screens();
		const existing = this.screensById.get(screen.id);
		const action: ScreenConfigurationAction = existing
			? {
					type: "update",
					screen_id: screen.id,
					patch: screenPatch(existing, screen),
				}
			: { type: "create", configuration: wireScreen(screen) };
		const outcome = await this.screenAction(action);
		const saved = outcome.screen as ScreenConfiguration | null;
		if (!saved) throw new Error("Screen update returned no screen");
		this.screensById.set(saved.id, saved);
		return saved;
	}

	async deleteScreen(id: string): Promise<void> {
		await this.screenAction({ type: "delete", screen_id: id });
		this.screensById.delete(id);
	}

	async setScreenPage(id: string, page: number): Promise<void> {
		await this.screenAction({ type: "set_page", screen_id: id, page });
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

	async setPlaybackPage(
		deskId: string,
		page: number,
		options: PlaybackPageSelectionOptions = {},
	) {
		const outcome = await this.controlDeskAction(deskId, {
			type: "set_page",
			page,
			existing_only: options.existingOnly ?? false,
		});
		return {
			desk_id: outcome.desk.id,
			page: outcome.page ?? page,
			event_sequence: outcome.event_sequence,
			page_creation_event_sequence: outcome.page_creation_event_sequence,
		};
	}

	async updateControlDesk(
		desk: ControlDesk,
		previous?: ControlDesk,
	): Promise<ControlDesk> {
		const outcome = await this.controlDeskAction(desk.id, {
			type: "update",
			patch: controlDeskPatch(previous, desk),
		});
		return outcome.desk as ControlDesk;
	}

	removeClient(deskId: string): Promise<void> {
		return this.transport.request(`/api/v1/clients/${deskId}`, {
			method: "DELETE",
		});
	}

	private screenAction(
		action: ScreenConfigurationAction,
	): Promise<ScreenConfigurationActionOutcome> {
		const request: ScreenConfigurationActionRequest = {
			request_id: crypto.randomUUID(),
			action,
		};
		return this.transport.request("/api/v2/screens/actions", {
			method: "POST",
			headers: { "content-type": "application/json" },
			body: JSON.stringify(request),
		});
	}

	private controlDeskAction(
		deskId: string,
		action: ControlDeskConfigurationAction,
	): Promise<ControlDeskConfigurationActionOutcome> {
		const request: ControlDeskConfigurationActionRequest = {
			request_id: crypto.randomUUID(),
			action,
		};
		return this.transport.request(`/api/v2/control-desks/${deskId}/actions`, {
			method: "POST",
			headers: { "content-type": "application/json" },
			body: JSON.stringify(request),
		});
	}
}

function wireScreen(screen: ScreenConfiguration): WireScreenConfiguration {
	return {
		...screen,
		playback_layout: screen.playback_layout ?? null,
	};
}

function screenPatch(
	current: ScreenConfiguration,
	next: ScreenConfiguration,
): ScreenConfigurationPatch {
	const changed = <T>(before: T, after: T) =>
		JSON.stringify(before) === JSON.stringify(after) ? null : after;
	return {
		name: changed(current.name, next.name),
		layout: changed(current.layout, next.layout),
		show_dock: changed(current.show_dock, next.show_dock),
		show_playbacks: changed(current.show_playbacks, next.show_playbacks),
		playback_count: changed(current.playback_count, next.playback_count),
		playback_rows: changed(current.playback_rows, next.playback_rows),
		first_playback_slot: changed(
			current.first_playback_slot,
			next.first_playback_slot,
		),
		page_mode: changed(current.page_mode, next.page_mode),
		show_page_controls: changed(
			current.show_page_controls,
			next.show_page_controls,
		),
		desired_open: changed(current.desired_open, next.desired_open),
		display_id:
			next.display_id == null
				? null
				: changed(current.display_id, next.display_id),
		clear_display_id: current.display_id != null && next.display_id == null,
		bounds: next.bounds == null ? null : changed(current.bounds, next.bounds),
		clear_bounds: current.bounds != null && next.bounds == null,
		fullscreen: changed(current.fullscreen, next.fullscreen),
		playback_layout:
			next.playback_layout == null
				? null
				: changed(
						current.playback_layout ?? null,
						next.playback_layout ?? null,
					),
		clear_playback_layout:
			current.playback_layout != null && next.playback_layout == null,
	};
}

function controlDeskPatch(
	current: ControlDesk | undefined,
	next: ControlDesk,
): ControlDeskConfigurationPatch {
	const changed = <T>(before: T | undefined, after: T) =>
		before === after ? null : after;
	return {
		name: changed(current?.name, next.name),
		osc_alias: changed(current?.osc_alias, next.osc_alias),
		columns: changed(current?.columns, next.columns),
		rows: changed(current?.rows, next.rows),
		buttons: changed(current?.buttons, next.buttons),
		playback_layout:
			next.playback_layout == null ||
			JSON.stringify(current?.playback_layout) ===
				JSON.stringify(next.playback_layout)
				? null
				: next.playback_layout,
	};
}
