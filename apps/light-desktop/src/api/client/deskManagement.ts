import type {
	SpeedGroupActionOutcome,
	SpeedGroupActionRequest,
} from "../../features/speedGroupRuntime/contracts";
import type {
	ConfigurationPatch,
	ConfigurationUpdateRequest,
	DeskLockConfigurationUpdateRequest,
	SoundToLightConfiguration,
	SpeedGroupLiveActionRequest,
	SpeedGroupSettingsUpdateRequest,
	UserCreateRequest,
} from "../generated/light-wire";
import {
	decodeSpeedGroupActionOutcome,
	encodeSpeedGroupActionRequest,
} from "../speedGroupRuntimeWire";
import type {
	CommandHistoryEntry,
	DeskConfiguration,
	DeskLockState,
	DeskUser,
	HighlightLookConfiguration,
	MatterBridgeStatus,
	OutputHealth,
	PoolPresentationConfiguration,
	ProgrammerState,
	SoundObservation,
	SoundToLightConfig,
	SpeedGroupActionInput,
	SpeedGroupId,
	SpeedGroupSoundState,
	SpeedGroupSource,
} from "../types";
import type { LiveClientTransport } from "./transport";
import { jsonRequest } from "./transport";

export interface ConfigurationSnapshot {
	configuration: DeskConfiguration;
	output_health: OutputHealth;
	matter: MatterBridgeStatus;
}

export interface ConfigurationUpdateResult {
	configuration: DeskConfiguration;
	requires_restart: boolean;
	matter: MatterBridgeStatus;
	request_id: string;
	replayed: boolean;
}

export interface DeskLockInput {
	message: string;
	wallpaper: string | null;
	unlock_mode: "button" | "pin";
	pin?: string;
}

export class DeskManagementApiClient {
	constructor(private readonly transport: LiveClientTransport) {}

	async speedGroupRuntimeLiveAction(
		request: SpeedGroupActionRequest,
	): Promise<SpeedGroupActionOutcome> {
		const wireRequest = encodeSpeedGroupActionRequest(request);
		const value = await this.transport.sendAction(
			{ type: "speed_group", request: wireRequest },
			wireRequest.request_id,
		);
		return decodeSpeedGroupActionOutcome(value, request);
	}

	configuration(): Promise<ConfigurationSnapshot> {
		return this.transport.request("/api/v2/configuration");
	}

	updateConfiguration(
		configuration: DeskConfiguration,
	): Promise<ConfigurationUpdateResult> {
		const request: ConfigurationUpdateRequest = {
			request_id: crypto.randomUUID(),
			patch: configurationPatch(configuration),
		};
		return this.transport.request(
			"/api/v2/configuration/update",
			jsonRequest("POST", request),
		);
	}

	updatePoolPresentation(
		poolPresentation: PoolPresentationConfiguration,
	): Promise<ConfigurationUpdateResult> {
		const request: ConfigurationUpdateRequest = {
			request_id: crypto.randomUUID(),
			patch: { pool_presentation: poolPresentation },
		};
		return this.transport.request(
			"/api/v2/configuration/update",
			jsonRequest("POST", request),
		);
	}

	matterStatus(): Promise<MatterBridgeStatus> {
		return this.transport.request("/api/v2/matter/status");
	}

	speedGroup(group: SpeedGroupId): Promise<SpeedGroupSoundState> {
		return this.transport.request(`/api/v2/speed-groups/${group}`);
	}

	updateSpeedGroup(
		group: SpeedGroupId,
		configuration: SoundToLightConfig,
		source: SpeedGroupSource = configuration.enabled
			? { type: "sound_to_light" }
			: { type: "manual" },
	) {
		const request: SpeedGroupSettingsUpdateRequest = {
			request_id: crypto.randomUUID(),
			source,
			configuration: soundConfiguration(configuration),
		};
		return this.transport.request<SpeedGroupSoundState>(
			`/api/v2/speed-groups/${group}/settings/update`,
			jsonRequest("POST", request),
		);
	}

	observeSpeedGroup(group: SpeedGroupId, observation: SoundObservation) {
		return this.transport.request<SpeedGroupSoundState>(
			`/api/v2/speed-groups/${group}/observations`,
			jsonRequest("POST", observation),
		);
	}

	speedGroupAction(group: SpeedGroupId, input: SpeedGroupActionInput) {
		const request: SpeedGroupLiveActionRequest = { ...input, bpm: undefined };
		return this.transport.request<SpeedGroupSoundState>(
			`/api/v2/speed-groups/${group}/actions`,
			jsonRequest("POST", request),
		);
	}

	shutdown(): Promise<{ shutting_down: boolean }> {
		return this.transport.request("/api/v2/shutdown", { method: "POST" });
	}

	deskLock(): Promise<DeskLockState> {
		return this.transport.request(this.deskLockPath());
	}

	configureDeskLock(input: DeskLockInput): Promise<DeskLockState> {
		const request: DeskLockConfigurationUpdateRequest = {
			request_id: crypto.randomUUID(),
			...input,
		};
		return this.transport.request(
			`${this.deskLockPath()}/update`,
			jsonRequest("POST", request),
		);
	}

	lockDesk(): Promise<DeskLockState> {
		return this.transport.request(`${this.deskLockPath()}/lock`);
	}

	unlockDesk(pin?: string): Promise<DeskLockState> {
		return this.transport.request(
			`${this.deskLockPath()}/unlock`,
			jsonRequest("POST", { pin }),
		);
	}

	commandHistory(): Promise<CommandHistoryEntry[]> {
		return this.transport.request("/api/v2/command-history");
	}

	async createUser(name: string): Promise<DeskUser> {
		const request: UserCreateRequest = {
			request_id: crypto.randomUUID(),
			name,
			enabled: true,
		};
		const response = await this.transport.request<{ user: DeskUser }>(
			"/api/v2/users/create",
			jsonRequest("POST", request),
		);
		return response.user;
	}

	auditEvents(after = 0) {
		return this.transport.request<
			Array<{ revision: number; kind: string; payload: unknown }>
		>(`/api/v2/audit?after=${after}`);
	}

	programmers(): Promise<ProgrammerState[]> {
		return this.transport.request("/api/v2/programmers");
	}

	clearProgrammer(sessionId: string): Promise<void> {
		return this.transport.request(`/api/v2/programmers/${sessionId}/clear`, {
			method: "POST",
		});
	}

	private deskLockPath() {
		const deskId = this.transport.currentDeskId();
		if (!deskId) throw new Error("A desk session is required");
		return `/api/v2/control-desks/${deskId}/desk-lock`;
	}
}

function soundConfiguration(
	configuration: SoundToLightConfig,
): SoundToLightConfiguration {
	return {
		analysis_mode: configuration.analysis_mode,
		frequency: configuration.frequency,
		input_gain_db: configuration.input_gain_db,
		confidence_threshold: configuration.confidence_threshold,
		smoothing: configuration.smoothing,
		minimum_bpm: configuration.minimum_bpm,
		maximum_bpm: configuration.maximum_bpm,
		signal_hold_millis: configuration.signal_hold_millis,
		multiplier: configuration.multiplier,
	};
}

function configurationPatch(
	configuration: DeskConfiguration,
): ConfigurationPatch & { highlight_look: HighlightLookConfiguration } {
	return {
		frame_rate_hz: configuration.frame_rate_hz,
		output_bind_ip: configuration.output_bind_ip,
		osc_bind: configuration.osc_bind,
		art_timecode_bind: configuration.art_timecode_bind,
		midi_inputs: configuration.midi_inputs,
		rtp_midi_bind: configuration.rtp_midi_bind,
		timecode_sources: configuration.timecode_sources,
		osc_timecode: configuration.osc_timecode,
		backup_retention: configuration.backup_retention,
		autosave_interval_seconds: configuration.autosave_interval_seconds,
		programmer_fade_millis: configuration.programmer_fade_millis,
		command_line_at_uses_programmer_fade:
			configuration.command_line_at_uses_programmer_fade,
		sequence_master_fade_millis: configuration.sequence_master_fade_millis,
		preload_programmer_changes: configuration.preload_programmer_changes,
		preload_physical_playback_actions:
			configuration.preload_physical_playback_actions,
		preload_virtual_playback_actions:
			configuration.preload_virtual_playback_actions,
		patch_preview_highlight_dmx:
			configuration.patch_preview_highlight_dmx ?? false,
		highlight_look: configuration.highlight_look ?? {
			intensity: 1,
			color: "white",
			iris: null,
			zoom: null,
			focus: null,
			frost: null,
			compatibility: "semantic",
		},
		matter_enabled: configuration.matter_enabled ?? false,
		...(configuration.pool_presentation
			? { pool_presentation: configuration.pool_presentation }
			: {}),
		file_manager_system_picker_fallback:
			configuration.file_manager_system_picker_fallback,
		file_manager_roots: configuration.file_manager_roots,
	};
}
