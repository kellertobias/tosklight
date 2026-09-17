import type { NativeHardwareBridge } from "../transport/nativeBridge";
import type { OscBridge } from "../transport/oscBridge";
import { type SettingsStorage, saveControllerSettings } from "./settings";
import type { ControllerSettings, DeviceStatus } from "./types";

export const nativeStatusIntervalMs = 1000;

export interface LinkTransports {
	bridge: OscBridge;
	nativeBridge: NativeHardwareBridge;
	storage: SettingsStorage;
}

/** What one connect attempt may change; every write is ignored once it is superseded. */
export interface LinkAttempt {
	isCurrent: () => boolean;
	setLinkError: (message: string) => void;
	setDevice: (status: DeviceStatus) => void;
	stopPolling: () => void;
	startPolling: (refresh: () => Promise<void>) => void;
}

/**
 * Opens the link for one mode. Both paths are torn down first so no stale OSC
 * subscription or native session keeps delivering into the new mode.
 */
export async function openLink(
	target: ControllerSettings,
	{ bridge, nativeBridge, storage }: LinkTransports,
	attempt: LinkAttempt,
): Promise<void> {
	attempt.stopPolling();
	await nativeBridge.close();
	await bridge.disconnect?.().catch(() => undefined);
	if (!attempt.isCurrent()) return;
	// The OSC subscription also carries the desk feedback mirrored in Native Hardware mode.
	try {
		await bridge.connect({
			host: target.host,
			port: target.port,
			desk: target.desk,
		});
	} catch (error) {
		if (!attempt.isCurrent()) return;
		attempt.setLinkError(
			`cannot open the OSC link to ${target.host}:${target.port}: ${errorText(error)}`,
		);
		if (target.mode === "native")
			attempt.setDevice({ state: "error", message: "no desk link" });
		return;
	}
	saveControllerSettings(storage, target);
	if (target.mode !== "native" || !attempt.isCurrent()) return;
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
	if (!attempt.isCurrent()) {
		await nativeBridge.close();
		return;
	}
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
