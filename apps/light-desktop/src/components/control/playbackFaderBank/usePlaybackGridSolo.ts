import {
	useEffect,
	useMemo,
	useSyncExternalStore,
	type SyntheticEvent,
	type PointerEvent,
	type KeyboardEvent,
	type MouseEvent,
} from "react";
import {
	playbackAddressKey,
} from "../../../api/playbackAddress";
import { usePlaybackSoloEditorRegistry } from "../../../features/playbackSolo/PlaybackSoloEditorContext";
import { usePlaybackSoloDraft } from "./usePlaybackSoloDraft";
import { usePlaybackSoloOptional } from "../../../features/playbackSolo/PlaybackSoloContext";
import type { PlaybackBankController } from "./controller";
import { playbackCellKey } from "./projection";
import { soloFenceClassMap } from "./soloFence";
import type { PlaybackSlotProjection } from "./types";
const noSubscribe = () => () => {};
const noRegions = () => null;
const notSaving = () => false;

export function usePlaybackGridSolo(
	controller: Pick<
		PlaybackBankController,
		"surfaceId" | "activePageNumber" | "slots" | "columns"
	>,
	surfaceId = controller.surfaceId,
) {
	const capability = usePlaybackSoloOptional();
	const registry = usePlaybackSoloEditorRegistry();
	useEffect(() => capability?.activate(), [capability?.authorityGeneration]);
	const regions = useSyncExternalStore(
		capability?.subscribe ?? noSubscribe,
		capability?.getRegions ?? noRegions,
		capability?.getRegions ?? noRegions,
	);
	const saving = useSyncExternalStore(
		capability?.subscribe ?? noSubscribe,
		capability?.isSaving ?? notSaving,
		capability?.isSaving ?? notSaving,
	);
	const draft = usePlaybackSoloDraft(capability, regions);
	const {
		creating, selecting, selected, setSelected, name, setName, error,
		managing, setManaging, cancel, create, beginEdit,
	} = draft;
	const toggle = (target: EventTarget | null) => {
		if (!(target instanceof Element) || controller.activePageNumber == null)
			return;
		const position = Number(
			target.closest<HTMLElement>("[data-playback-slot]")?.dataset.playbackSlot,
		);
		if (!controller.slots.some((cell) => cell.slot === position)) return;
		const address = { page: controller.activePageNumber, position };
		const key = playbackAddressKey(address);
		setSelected((current) =>
			current.some((item) => playbackAddressKey(item) === key)
				? current.filter((item) => playbackAddressKey(item) !== key)
				: [...current, address],
		);
	};
	const block = (event: SyntheticEvent) => {
		if (!selecting) return false;
		event.preventDefault();
		event.stopPropagation();
		return true;
	};
	const pointerDown = (event: PointerEvent) => {
		if (block(event) && event.button === 0) toggle(event.target);
	};
	const click = (event: MouseEvent) => {
		// Pointer presses already selected the cell. AX activation emits click alone.
		if (block(event) && event.detail === 0) toggle(event.target);
	};
	const keyDown = (event: KeyboardEvent) => {
		if (event.key === "Tab") return;
		if (
			block(event) &&
			!event.repeat &&
			(event.key === " " || event.key === "Enter")
		)
			toggle(event.target);
	};
	const fenceRegions = useMemo(
		() =>
			selecting
				? [{ id: "draft", name: "Selection", members: selected }]
				: (regions ?? []),
		[selecting, selected, regions],
	);
	const fences = useMemo(
		() =>
			soloFenceClassMap(
				controller.slots,
				controller.activePageNumber,
				controller.columns,
				fenceRegions,
			),
		[
			controller.slots,
			controller.activePageNumber,
			controller.columns,
			fenceRegions,
		],
	);
	const classes = (cell: PlaybackSlotProjection) => {
		const chosen = selected.some(
			(address) =>
				address.page === controller.activePageNumber &&
				address.position === cell.slot,
		);
		return `${fences.get(playbackCellKey(cell)) ?? ""} ${chosen ? "playback-solo-selected" : ""}`.trim();
	};
	const memberPositions = useMemo(() => new Set(
		(regions ?? []).flatMap(region => region.members
			.filter(address => address.page === controller.activePageNumber)
			.map(address => address.position)),
	), [regions, controller.activePageNumber]);
	const result = {
		memberPositions,
		fenceRegions,
		available: capability?.available ?? false,
		ready: regions !== null,
		saving: creating || saving,
		error: error ?? capability?.error,
		selecting,
		selected,
		name,
		setName,
		managing,
		setManaging,
		cancel,
		create,
		classes,
		block,
		pointerDown,
		click,
		keyDown,
		begin: () => beginEdit(),
		beginEdit,
		save: create,
	};
	useEffect(() => {
		if (!registry || !surfaceId) return;
		return registry.register(surfaceId, result);
	}, [
		registry, surfaceId, capability?.authorityGeneration, capability?.available,
		capability?.error, regions, selecting, selected, name, creating, saving, error,
	]);
	return result;
}
export type PlaybackGridSolo = ReturnType<typeof usePlaybackGridSolo>;
