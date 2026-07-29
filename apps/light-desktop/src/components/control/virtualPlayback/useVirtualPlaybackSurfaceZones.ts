import { useCallback, useEffect, useRef, useSyncExternalStore } from "react";
import type { VirtualPlaybackZone } from "../../../features/virtualPlaybackZones/contracts";
import { useVirtualPlaybackZones } from "../../../features/virtualPlaybackZones/VirtualPlaybackZonesContext";

interface SurfaceZoneOptions {
	surfaceId: string;
	active: boolean;
	authorityReady: boolean;
	/** Layout identity is intentionally ignored; zones are show-global. */
	pageMode?: unknown;
}

/** Projects the show-global zone set into a pane; local errors never retrigger reads. */
export function useVirtualPlaybackSurfaceZones({
	active,
	authorityReady,
}: SurfaceZoneOptions) {
	const capability = useVirtualPlaybackZones();
	const capabilityRef = useRef(capability);
	capabilityRef.current = capability;
	const subscribe = useCallback(
		(listener: () => void) => capabilityRef.current.subscribe(listener),
		[capability.authorityGeneration],
	);
	const getSnapshot = useCallback(
		() => capabilityRef.current.getZones(),
		[capability.authorityGeneration],
	);
	const zones = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
	const getSaving = useCallback(
		() => capabilityRef.current.isSaving(),
		[capability.authorityGeneration],
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
		return source.activate();
	}, [
		active,
		authorityReady,
		capability.authorityId,
		capability.authorityGeneration,
		capability.available,
	]);

	useEffect(() => {
		const source = capabilityRef.current;
		if (!active || !authorityReady || !source.available || !source.authorityId)
			return;
		if (source.getZones() !== null) return;
		void source.load();
	}, [
		active,
		authorityReady,
		capability.authorityId,
		capability.authorityGeneration,
		capability.available,
	]);
	const persist = useCallback(
		async (zones: readonly VirtualPlaybackZone[]) => {
			if (!active || !authorityReady) return false;
			const source = capabilityRef.current;
			const authorityId = source.authorityId;
			const authorityGeneration = source.authorityGeneration;
			if (!authorityId || source.isSaving()) return false;
			const saved = await source.save(zones);
			if (!saved) return false;
			const current = capabilityRef.current;
			return (
				current.authorityId === authorityId &&
				current.authorityGeneration === authorityGeneration
			);
		},
		[active, authorityReady],
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
