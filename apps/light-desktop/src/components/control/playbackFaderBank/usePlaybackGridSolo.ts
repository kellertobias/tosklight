import {
	useEffect,
	useMemo,
	useState,
	useRef,
	useSyncExternalStore,
	type SyntheticEvent,
	type PointerEvent,
	type KeyboardEvent,
} from "react";
import {
	playbackAddressKey,
	type PlaybackAddressValue,
} from "../../../api/playbackAddress";
import { usePlaybackSoloOptional } from "../../../features/playbackSolo/PlaybackSoloContext";
import type { PlaybackBankController } from "./controller";
import { soloFenceClasses } from "./soloFence";
import type { PlaybackSlotProjection } from "./types";
const noSubscribe = () => () => {};
const noRegions = () => null;

export function usePlaybackGridSolo(controller: PlaybackBankController) {
	const capability = usePlaybackSoloOptional();
	useEffect(() => capability?.activate(), [capability?.authorityGeneration]);
	const regions = useSyncExternalStore(
		capability?.subscribe ?? noSubscribe,
		capability?.getRegions ?? noRegions,
		capability?.getRegions ?? noRegions,
	);
	const [creating, setCreating] = useState(false);
	const createPending = useRef(false);
	const [selecting, setSelecting] = useState(false);
	const [selected, setSelected] = useState<PlaybackAddressValue[]>([]);
	const [name, setName] = useState("");
	const [error, setError] = useState<string | null>(null);
	const [managing, setManaging] = useState(false);
	useEffect(() => {
		setSelecting(false);
		setSelected([]);
		setManaging(false);
	}, [capability?.authorityGeneration]);
	const cancel = () => {
		setSelecting(false);
		setSelected([]);
		setError(null);
	};
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
	const keyDown = (event: KeyboardEvent) => {
		if (event.key === "Tab") return;
		if (
			block(event) &&
			!event.repeat &&
			(event.key === " " || event.key === "Enter")
		)
			toggle(event.target);
	};
	const create = async () => {
		if (!capability || !regions || createPending.current) return;
		if (!name.trim()) {
			setError("Enter a Solo Region name");
			return;
		}
		if (selected.length < 2) {
			setError("Select at least two Playback addresses");
			return;
		}
		setError(null);
		createPending.current = true;
		setCreating(true);
		try {
			const next = await capability.save([
				...regions,
				{ id: crypto.randomUUID(), name: name.trim(), members: selected },
			]);
			if (next) {
				cancel();
				setName("");
			}
		} finally {
			createPending.current = false;
			setCreating(false);
		}
	};
	const fences = useMemo(
		() =>
			new Map(
				controller.slots.map((cell) => [
					cell.slot,
					controller.activePageNumber == null
						? ""
						: soloFenceClasses(
								cell,
								controller.slots,
								controller.activePageNumber,
								controller.columns,
								selecting
									? [{ id: "draft", name: "Selection", members: selected }]
									: (regions ?? []),
							),
				]),
			),
		[
			controller.slots,
			controller.activePageNumber,
			controller.columns,
			regions,
			selecting,
			selected,
		],
	);
	const classes = (cell: PlaybackSlotProjection) => {
		const chosen = selected.some(
			(address) =>
				address.page === controller.activePageNumber &&
				address.position === cell.slot,
		);
		return `${fences.get(cell.slot) ?? ""} ${chosen ? "playback-solo-selected" : ""}`.trim();
	};
	return {
		available: capability?.available ?? false,
		ready: regions !== null,
		saving: creating || (capability?.isSaving() ?? false),
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
		keyDown,
		begin: () => {
			setSelected([]);
			setError(null);
			setSelecting(true);
		},
	};
}
export type PlaybackGridSolo = ReturnType<typeof usePlaybackGridSolo>;
