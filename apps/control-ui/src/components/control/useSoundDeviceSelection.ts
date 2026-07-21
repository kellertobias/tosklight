import { type RefObject, useCallback, useEffect, useState } from "react";
import type { SpeedGroupId } from "../../api/types";
import {
	type AudioInputDevice,
	enumerateAudioInputs,
	type MicrophonePermission,
	microphonePermission,
} from "./soundToLightAnalyzer";
import {
	type SoundGroupMap,
	soundDeviceStorageKey,
	speedGroupIds,
} from "./soundToLightModel";

function browserLocalStorage(): Storage | null {
	const storage = globalThis.localStorage;
	return storage &&
		typeof storage.getItem === "function" &&
		typeof storage.setItem === "function"
		? storage
		: null;
}

export function useSoundDeviceSelection(
	deskId: string | null,
	mounted: RefObject<boolean>,
	enabled = true,
) {
	const [devices, setDevices] = useState<AudioInputDevice[]>([]);
	const [deviceIds, setDeviceIds] = useState<SoundGroupMap<string>>({});
	const [permission, setPermission] = useState<MicrophonePermission>("unknown");

	const refreshInputs = useCallback(async () => {
		if (!enabled) return;
		const [nextPermission, nextDevices] = await Promise.all([
			microphonePermission(),
			enumerateAudioInputs().catch(() => []),
		]);
		if (!mounted.current) return;
		setPermission(nextPermission);
		setDevices(nextDevices);
	}, [enabled, mounted]);

	useEffect(() => {
		if (!enabled) return;
		void refreshInputs();
		const changed = () => void refreshInputs();
		navigator.mediaDevices?.addEventListener?.("devicechange", changed);
		return () =>
			navigator.mediaDevices?.removeEventListener?.("devicechange", changed);
	}, [enabled, refreshInputs]);

	useEffect(() => {
		if (!enabled || !deskId) {
			setDeviceIds((current) =>
				Object.keys(current).length === 0 ? current : {},
			);
			return;
		}
		const mappings: SoundGroupMap<string> = {};
		const storage = browserLocalStorage();
		for (const group of speedGroupIds) {
			const selected = storage?.getItem(soundDeviceStorageKey(deskId, group));
			if (selected) mappings[group] = selected;
		}
		setDeviceIds(mappings);
	}, [deskId, enabled]);

	const setDevice = useCallback(
		(group: SpeedGroupId, deviceId: string) => {
			if (!enabled || !deskId) return;
			const key = soundDeviceStorageKey(deskId, group);
			const storage = browserLocalStorage();
			if (deviceId) storage?.setItem(key, deviceId);
			else storage?.removeItem(key);
			setDeviceIds((current) => {
				const next = { ...current };
				if (deviceId) next[group] = deviceId;
				else delete next[group];
				return next;
			});
		},
		[deskId, enabled],
	);

	return {
		devices,
		deviceIds,
		permission,
		setPermission,
		refreshInputs,
		setDevice,
	};
}
