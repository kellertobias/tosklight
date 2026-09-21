import type { NativeHardwareBridge } from "../transport/nativeBridge";
import type { NativeSimulatorBridge } from "../transport/nativeSimulatorBridge";
import type { OscBridge } from "../transport/oscBridge";
import { type SettingsStorage, saveControllerSettings } from "./settings";
import type { ControllerSettings, DeviceStatus } from "./types";

export const nativeStatusIntervalMs = 1000;

export interface LinkTransports {
	bridge: OscBridge;
	nativeBridge: NativeHardwareBridge;
	simulatorBridge: NativeSimulatorBridge;
	storage: SettingsStorage;
}

/** What one connect attempt may change; every write is ignored once it is superseded. */
export interface LinkAttempt {
	isCurrent: () => boolean;
	setLinkError: (message: string) => void;
	setDevice: (status: DeviceStatus) => void;
	setConnected: () => void;
	stopPolling: () => void;
	startPolling: (refresh: () => Promise<void>) => void;
}

/**
 * Opens the link for one mode. Both paths are torn down first so no stale OSC
 * subscription or native session keeps delivering into the new mode.
 */
export async function openLink(
	target: ControllerSettings,
	{ bridge, nativeBridge, simulatorBridge, storage }: LinkTransports,
	attempt: LinkAttempt,
): Promise<void> {
	attempt.stopPolling();
	await nativeBridge.close();
	await simulatorBridge.close();
	await bridge.disconnect?.().catch(() => undefined);
	if (!attempt.isCurrent()) return;
	if (target.mode === "osc") {
		try {
			await bridge.connect({
				host: target.host,
				port: target.port,
				desk: target.desk,
			});
			saveControllerSettings(storage, target);
		} catch (error) {
			if (attempt.isCurrent())
				attempt.setLinkError(
					`cannot open the OSC link to ${target.host}:${target.port}: ${errorText(error)}`,
				);
		}
		return;
	}
	if (target.mode === "native-simulator") {
		try {
			await simulatorBridge.open(target);
			if (!attempt.isCurrent()) {
				await simulatorBridge.close();
				return;
			}
			attempt.setConnected();
			attempt.setDevice({
				state: "connected",
				name: "Supervised simulator extension",
			});
			saveControllerSettings(storage, target);
		} catch (error) {
			if (attempt.isCurrent()) {
				attempt.setDevice({ state: "error", message: errorText(error) });
				attempt.setLinkError(
					`cannot open the native simulator relay at ${target.host}:${target.simulatorPort}: ${errorText(error)}`,
				);
			}
		}
		return;
	}
	try {
		await nativeBridge.open(target);
	} catch (error) {
		if (attempt.isCurrent())
			attempt.setDevice({
				state: "error",
				message: `cannot reach the desk at ${target.host}:${target.serverPort}: ${errorText(error)}`,
			});
		return;
	}
	saveControllerSettings(storage, target);
	if (!attempt.isCurrent()) {
		await nativeBridge.close();
		return;
	}
	attempt.setConnected();
	const refresh = async () => {
		const status = await nativeBridge.status();
		if (attempt.isCurrent()) attempt.setDevice(status);
	};
	attempt.startPolling(refresh);
	await refresh();
}

function errorText(error: unknown): string {
	return error instanceof Error ? error.message : String(error);
}
