import {
	useCallback,
	useEffect,
	useRef,
	useSyncExternalStore,
} from "react";
import type { VirtualPlaybackZone } from "../../../features/virtualPlaybackZones/contracts";
import { useVirtualPlaybackZones } from "../../../features/virtualPlaybackZones/VirtualPlaybackZonesContext";

interface SurfaceZoneOptions {
	surfaceId: string;
	active: boolean;
	authorityReady: boolean;
}

/** Loads one surface only for a ready desk authority; local errors never retrigger reads. */
export function useVirtualPlaybackSurfaceZones({
	surfaceId,
	active,
	authorityReady,
}: SurfaceZoneOptions) {
	const capability = useVirtualPlaybackZones();
	const capabilityRef = useRef(capability);
	capabilityRef.current = capability;
	const subscribe = useCallback(
		(listener: () => void) =>
			capabilityRef.current.subscribeSurface(surfaceId, listener),
		[capability.authorityGeneration, surfaceId],
	);
	const getSnapshot = useCallback(
		() => capabilityRef.current.getSurface(surfaceId),
		[capability.authorityGeneration, surfaceId],
	);
	const zones = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
	const getSaving = useCallback(
		() => capabilityRef.current.isSavingSurface(surfaceId),
		[capability.authorityGeneration, surfaceId],
	);
	const saving = useSyncExternalStore(subscribe, getSaving, getSaving);
	const ready =
		active &&
		authorityReady &&
		capability.available &&
		capability.authorityId !== null &&
		zones !== null;

	useEffect(() => {
		const source = capabilityRef.current;
		if (!active || !authorityReady || !source.available || !source.authorityId)
			return;
		if (source.getSurface(surfaceId) !== null) return;
		void source.loadSurface(surfaceId);
	}, [
		active,
		authorityReady,
		capability.authorityId,
		capability.authorityGeneration,
		capability.available,
		surfaceId,
	]);
	const persist = useCallback(
		async (zones: readonly VirtualPlaybackZone[]) => {
			if (!active || !authorityReady) return false;
			const source = capabilityRef.current;
			const authorityId = source.authorityId;
			const authorityGeneration = source.authorityGeneration;
			if (!authorityId || source.isSavingSurface(surfaceId)) return false;
			const saved = await source.saveSurface(surfaceId, zones);
			if (!saved) return false;
			const current = capabilityRef.current;
			return (
				current.authorityId === authorityId &&
				current.authorityGeneration === authorityGeneration
			);
		},
		[active, authorityReady, surfaceId],
	);

	return {
		ready,
		saving,
		zones: ready ? zones : EMPTY_ZONES,
		error: capability.error,
		persist,
	};
}

const EMPTY_ZONES: readonly VirtualPlaybackZone[] = [];
