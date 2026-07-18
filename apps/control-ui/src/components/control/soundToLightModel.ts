import type { SpeedGroupId } from "../../api/types";

export const speedGroupIds: SpeedGroupId[] = ["A", "B", "C", "D", "E"];

export type SoundGroupMap<T> = Partial<Record<SpeedGroupId, T>>;

export function soundDeviceStorageKey(deskId: string, group: SpeedGroupId) {
	return `light.sound-to-light.device.${deskId}.${group}`;
}

export function soundToLightErrorMessage(reason: unknown) {
	return reason instanceof Error ? reason.message : String(reason);
}
