import type { Page } from "@playwright/test";
import type { OutputRuntimeActionOutcome } from "../../../apps/light-desktop/src/features/outputRuntime/contracts";
import type { ApiDriver } from "../core/api";
import type { DeskDriver } from "../core/desk";
import { BrowserNetworkOutput } from "./networkOutputScenario";
import { setOutputRuntime } from "./outputRuntime";

export interface PauseDynamicsControl {
	/** Playback configured with the Grand Master target. */
	readonly playbackNumber: number;
	/** Button configured with the `pause_dynamics` action. */
	readonly button: number;
}

/** Global output actions with explicit authority and control-surface semantics. */
export class BrowserOutput {
	readonly network: BrowserNetworkOutput;

	constructor(
		private readonly api: ApiDriver,
		private readonly desk: DeskDriver,
		page: Page,
	) {
		this.network = new BrowserNetworkOutput(api, page, desk);
	}

	async grandMaster(level: number): Promise<OutputRuntimeActionOutcome> {
		await this.desk.recordStep(
			"OUTPUT · GRAND MASTER",
			`Set the desk Grand Master to ${Math.round(level * 100)}%.`,
		);
		return setOutputRuntime(this.api, {
			surface: "api",
			showId: await this.activeShowId(),
			grandMaster: level,
		});
	}

	async blackout(enabled: boolean): Promise<OutputRuntimeActionOutcome> {
		await this.desk.recordStep(
			enabled ? "OUTPUT · BLACKOUT" : "OUTPUT · RELEASE BLACKOUT",
			enabled ? "Black out routed output." : "Release the desk blackout.",
		);
		return setOutputRuntime(this.api, {
			surface: "api",
			showId: await this.activeShowId(),
			blackout: enabled,
		});
	}

	/**
	 * Toggles paused dynamics through a real Grand Master playback button. The caller names the
	 * configured control so setup remains visible and this helper cannot invent a hidden global
	 * mutation route.
	 */
	async togglePausedDynamics(control: PauseDynamicsControl): Promise<void> {
		assertControl(control);
		await this.desk.recordStep(
			"OUTPUT · PAUSE DYNAMICS",
			`Operate Pause Dynamics on playback ${control.playbackNumber}, button ${control.button}.`,
		);
		await this.api.playbackNumberAction(control.playbackNumber, "button", {
			button: control.button,
			pressed: true,
		});
	}

	private async activeShowId(): Promise<string> {
		const bootstrap = await this.api.request<{
			active_show: { id: string } | null;
		}>("GET", "/api/v2/bootstrap", undefined, false);
		if (!bootstrap.active_show) throw new Error("No active show");
		return bootstrap.active_show.id;
	}
}

function assertControl(control: PauseDynamicsControl): void {
	for (const [label, value] of [
		["Playback number", control.playbackNumber],
		["Playback button", control.button],
	] as const) {
		if (!Number.isInteger(value) || value < 1)
			throw new Error(`${label} must be a positive integer`);
	}
}
