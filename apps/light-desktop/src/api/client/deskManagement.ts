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

export interface ExtensionPackageSnapshot {
	id: string | null;
	name: string | null;
	version: string | null;
	directory: string;
	package_digest: string | null;
	readiness: string;
	locally_approved_unsigned: boolean;
	diagnostics: Array<{ code: string; detail: string }>;
}

export interface ExtensionInstanceSnapshot {
	id: string;
	extension_id: string;
	package_digest: string;
	executable: string;
	state: string;
	last_error: string | null;
	launches: number;
	crashes: number;
	protocol_errors: number;
	inbound_drops: number;
	outbound_drops: number;
}

export interface ExtensionRuntimeSnapshot {
	extensions_directory: string;
	configuration_path: string;
	configuration_diagnostic: string | null;
	packages: ExtensionPackageSnapshot[];
	instances: ExtensionInstanceSnapshot[];
	instance_diagnostics: Array<{
		instance_id: string;
		code: string;
		detail: string;
	}>;
}

export type UsbDmxDriverKind = "open_dmx" | "enttec_usb_pro_v144";

export interface UsbDeviceIdentity {
	vendor_id: number;
	product_id: number;
	manufacturer?: string | null;
	product?: string | null;
	usb_serial?: string | null;
	widget_serial?: string | null;
	port_topology_hint?: string | null;
}

export interface UsbDmxEndpoint {
	endpoint_id: string;
	driver: UsbDmxDriverKind;
	identity: UsbDeviceIdentity;
	enabled: boolean;
}

export interface UsbDmxEndpointSnapshot {
	document: { revision: number; endpoints: UsbDmxEndpoint[] };
	diagnostics: Array<{
		endpoint_id: string;
		code: string;
		message: string;
		dropped_frames: number;
		driver_health?: {
			online: boolean;
			reconnecting: boolean;
			accepted_frames: number;
			reconnect_attempts: number;
			last_error?: string | null;
		} | null;
	}>;
	discovered_devices: Array<{
		port_name: string;
		identity: UsbDeviceIdentity;
	}>;
	discovery_error?: string | null;
	configuration_error?: string | null;
	request_id?: string | null;
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

	extensions(): Promise<ExtensionRuntimeSnapshot> {
		return this.transport.request("/api/v2/extensions");
	}

	rescanExtensions(): Promise<ExtensionRuntimeSnapshot> {
		return this.transport.request(
			"/api/v2/extensions/rescan",
			jsonRequest("POST", { request_id: crypto.randomUUID() }),
		);
	}

	usbDmxEndpoints(): Promise<UsbDmxEndpointSnapshot> {
		return this.transport.request("/api/v2/usb-dmx/endpoints");
	}

	updateUsbDmxEndpoints(
		expectedRevision: number,
		action:
			| { action: "upsert"; endpoint: UsbDmxEndpoint }
			| { action: "remove"; endpoint_id: string }
			| { action: "reset_malformed" },
	): Promise<UsbDmxEndpointSnapshot> {
		return this.transport.request(
			"/api/v2/usb-dmx/endpoints/update",
			jsonRequest("POST", {
				request_id: crypto.randomUUID(),
				expected_revision: expectedRevision,
				action,
			}),
		);
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
): ConfigurationPatch {
	return {
		frame_rate_hz: configuration.frame_rate_hz,
		output_bind_ip: configuration.output_bind_ip,
		osc_bind: configuration.osc_bind,
		art_timecode_bind: configuration.art_timecode_bind,
		timecode_source: configuration.timecode_source,
		timecode_frame_rate: configuration.timecode_frame_rate,
		timecode_external_loss_policy: configuration.timecode_external_loss_policy,
		timecode_external_loss_timeout_millis:
			configuration.timecode_external_loss_timeout_millis,
		osc_timecode: configuration.osc_timecode,
		internal_audio_library_roots: configuration.internal_audio_library_roots,
		internal_audio_output_devices: configuration.internal_audio_output_devices,
		backup_retention: configuration.backup_retention,
		autosave_interval_seconds: configuration.autosave_interval_seconds,
		programmer_fade_millis: configuration.programmer_fade_millis,
		command_line_at_uses_programmer_fade:
			configuration.command_line_at_uses_programmer_fade,
		sequence_master_fade_millis: configuration.sequence_master_fade_millis,
		release_fade_millis: configuration.release_fade_millis,
		cuelist_auto_off_at_zero_default:
			configuration.cuelist_auto_off_at_zero_default,
		cuelist_auto_off_flash_release_default:
			configuration.cuelist_auto_off_flash_release_default,
		start_after_first_recording: configuration.start_after_first_recording,
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
