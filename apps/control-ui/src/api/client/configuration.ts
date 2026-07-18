import type {
	DeskConfiguration,
	DeskLockState,
	MatterBridgeStatus,
	OutputHealth,
	SoundObservation,
	SoundToLightConfig,
	SpeedGroupActionInput,
	SpeedGroupId,
	SpeedGroupSoundState,
} from "../types";
import type { ClientTransport } from "./transport";
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
}

export interface DeskLockInput {
	message: string;
	wallpaper: string | null;
	unlock_mode: "button" | "pin";
	pin?: string;
}

export class ConfigurationApiClient {
	constructor(private readonly transport: ClientTransport) {}

	configuration(): Promise<ConfigurationSnapshot> {
		return this.transport.request("/api/v1/configuration", {}, false);
	}

	updateConfiguration(configuration: DeskConfiguration) {
		return this.transport.request<ConfigurationUpdateResult>(
			"/api/v1/configuration",
			jsonRequest("PUT", configuration),
		);
	}

	matterStatus(): Promise<MatterBridgeStatus> {
		return this.transport.request("/api/v1/matter/status");
	}

	speedGroup(group: SpeedGroupId): Promise<SpeedGroupSoundState> {
		return this.transport.request(`/api/v1/speed-groups/${group}`);
	}

	updateSpeedGroup(group: SpeedGroupId, configuration: SoundToLightConfig) {
		return this.transport.request<SpeedGroupSoundState>(
			`/api/v1/speed-groups/${group}`,
			jsonRequest("PUT", configuration),
		);
	}

	observeSpeedGroup(group: SpeedGroupId, observation: SoundObservation) {
		return this.transport.request<SpeedGroupSoundState>(
			`/api/v1/speed-groups/${group}/observation`,
			jsonRequest("POST", observation),
		);
	}

	speedGroupAction(group: SpeedGroupId, input: SpeedGroupActionInput) {
		return this.transport.request<SpeedGroupSoundState>(
			`/api/v1/speed-groups/${group}/action`,
			jsonRequest("POST", input),
		);
	}

	shutdown(): Promise<{ shutting_down: boolean }> {
		return this.transport.request("/api/v1/shutdown", { method: "POST" });
	}

	deskLock(): Promise<DeskLockState> {
		return this.transport.request("/api/v1/desk-lock");
	}

	configureDeskLock(input: DeskLockInput): Promise<DeskLockState> {
		return this.transport.request(
			"/api/v1/desk-lock",
			jsonRequest("PUT", input),
		);
	}

	lockDesk(): Promise<DeskLockState> {
		return this.transport.request(
			"/api/v1/desk-lock/lock",
			jsonRequest("POST", {}),
		);
	}

	unlockDesk(pin?: string): Promise<DeskLockState> {
		return this.transport.request(
			"/api/v1/desk-lock/unlock",
			jsonRequest("POST", { pin }),
		);
	}
}
