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
	ScreenConfigurationCreateRequest,
	ScreenConfigurationDeleteRequest,
	ScreenConfigurationPatch,
	ScreenConfigurationUpdateRequest,
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
		const value = await this.transport.sendAction(
			{ type: "playback", request },
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

	async removeClient(deskId: string): Promise<void> {
		await this.controlDeskAction(deskId, { type: "remove_client" });
	}

	private screenAction(
		action: ScreenConfigurationAction,
	): Promise<ScreenConfigurationActionOutcome> {
		const requestId = crypto.randomUUID();
		let path: string;
		let request:
			| ScreenConfigurationActionRequest
			| ScreenConfigurationCreateRequest
			| ScreenConfigurationUpdateRequest
			| ScreenConfigurationDeleteRequest;
		switch (action.type) {
			case "create":
				path = "/api/v2/screens/create";
				request = {
					request_id: requestId,
					configuration: action.configuration,
				};
				break;
			case "update":
				path = `/api/v2/screens/${action.screen_id}/update`;
				request = { request_id: requestId, patch: action.patch };
				break;
			case "delete":
				path = `/api/v2/screens/${action.screen_id}/delete`;
				request = { request_id: requestId };
				break;
			case "set_page":
				path = "/api/v2/screens/actions";
				request = { request_id: requestId, action };
				break;
		}
		return this.transport.request(path, {
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
		content: changed(current.content, next.content),
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
