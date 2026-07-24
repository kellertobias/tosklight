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
	const [deviceId, setDeviceId] = useState("");
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
			setDeviceId("");
			setDeviceIds((current) =>
				Object.keys(current).length === 0 ? current : {},
			);
			return;
		}
		const storage = browserLocalStorage();
		const selected =
			storage?.getItem(soundDeviceStorageKey(deskId)) ??
			speedGroupIds
				.map((group) =>
					storage?.getItem(soundDeviceStorageKey(deskId, group)),
				)
				.find(Boolean) ??
			"";
		if (selected) {
			storage?.setItem(soundDeviceStorageKey(deskId), selected);
		}
		setDeviceId(selected);
		setDeviceIds(
			selected
				? (Object.fromEntries(
						speedGroupIds.map((group) => [group, selected]),
					) as SoundGroupMap<string>)
				: {},
		);
	}, [deskId, enabled]);

	const setDeskDevice = useCallback(
		(nextDeviceId: string) => {
			if (!enabled || !deskId) return;
			const key = soundDeviceStorageKey(deskId);
			const storage = browserLocalStorage();
			if (nextDeviceId) storage?.setItem(key, nextDeviceId);
			else storage?.removeItem(key);
			setDeviceId(nextDeviceId);
			setDeviceIds(
				nextDeviceId
					? (Object.fromEntries(
							speedGroupIds.map((group) => [group, nextDeviceId]),
						) as SoundGroupMap<string>)
					: {},
			);
		},
		[deskId, enabled],
	);
	const setDevice = useCallback(
		(_group: SpeedGroupId, nextDeviceId: string) =>
			setDeskDevice(nextDeviceId),
		[setDeskDevice],
	);

	return {
		devices,
		deviceId,
		deviceIds,
		permission,
		setPermission,
		refreshInputs,
		setDevice,
		setDeskDevice,
	};
}
