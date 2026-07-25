import { useEffect, useMemo } from "react";
import {
	usePlaybackDeskView,
	usePlaybackProjectionMap,
	usePlaybackRuntimeActions,
	usePlaybackRuntimeStatus,
} from "../playbackRuntime/PlaybackRuntimeView";
import { usePlaybackPagesView } from "../playbackTopology/PlaybackTopologyView";
import {
	demoFaderLevel,
	demoMappedPlaybackNumbers,
	demoSlotPlaybackNumbers,
} from "./demoPlaybackMapping";
import { HeldDemoButtons } from "./heldDemoButtons";

export interface DemoPlaybackControlsValue {
	/** True only once every portable, desk, runtime, and action authority resolved. */
	ready: boolean;
	status: DemoPlaybackControlsStatus;
	playbackNumber(slot: number): number | null;
	faderLevel(slot: number): number | null;
	press(slot: number, button: number): void;
	release(slot: number, button: number): void;
	setMaster(slot: number, value: number): void;
}

export type DemoPlaybackControlsStatus =
	| { kind: "ready" }
	| { kind: "loading"; message: string }
	| { kind: "error"; message: string };

/**
 * Reads the demo desk from the portable Page assignments and the exact active
 * desk Page, subscribes to nothing but the mapped Playback numbers, and refuses
 * every send while either authority is still loading.
 */
export function useDemoPlaybackControls(): DemoPlaybackControlsValue {
	const pagesView = usePlaybackPagesView();
	const playbackDesk = usePlaybackDeskView();
	const runtimeActions = usePlaybackRuntimeActions();
	const runtimeStatus = usePlaybackRuntimeStatus();
	const activePage = pagesView.ready ? playbackDesk?.active_page ?? null : null;
	const mapped = useMemo(
		() => demoSlotPlaybackNumbers(pagesView.pages, activePage),
		[activePage, pagesView.pages],
	);
	const numbers = useMemo(() => demoMappedPlaybackNumbers(mapped), [mapped]);
	const projections = usePlaybackProjectionMap(numbers);
	const projectionsReady = numbers.every(
		(playbackNumber) => projections.get(playbackNumber) !== undefined,
	);
	const status = demoPlaybackControlsStatus({
		pagesReady: pagesView.ready,
		pagesError: pagesView.error,
		deskReady: playbackDesk !== null,
		runtimeStatus,
		projectionsReady,
		actionsReady: runtimeActions !== null,
	});
	const ready = status.kind === "ready";
	const held = useMemo(
		() => new HeldDemoButtons(runtimeActions),
		[runtimeActions],
	);
	useEffect(() => {
		if (!ready) held.releaseAll();
	}, [held, ready]);
	useEffect(() => () => held.releaseAll(), [held]);
	return useMemo(() => {
		const resolve = (slot: number) =>
			ready ? (mapped.get(slot) ?? null) : null;
		return {
			ready,
			status,
			playbackNumber: resolve,
			faderLevel(slot) {
				const playbackNumber = resolve(slot);
				return playbackNumber == null
					? null
					: demoFaderLevel(projections.get(playbackNumber));
			},
			press(slot, button) {
				const playbackNumber = resolve(slot);
				if (playbackNumber == null) return;
				held.press(slot, playbackNumber, button);
			},
			release(slot, button) {
				held.releaseButton(slot, button);
			},
			setMaster(slot, value) {
				const playbackNumber = resolve(slot);
				if (playbackNumber == null) return;
				void runtimeActions?.poolPlaybackAction(playbackNumber, "master", {
					value,
					surface: "physical",
				});
			},
		};
	}, [held, mapped, projections, ready, runtimeActions, status]);
}

interface DemoPlaybackAuthority {
	pagesReady: boolean;
	pagesError: Error | null;
	deskReady: boolean;
	runtimeStatus: ReturnType<typeof usePlaybackRuntimeStatus>;
	projectionsReady: boolean;
	actionsReady: boolean;
}

function demoPlaybackControlsStatus(
	authority: DemoPlaybackAuthority,
): DemoPlaybackControlsStatus {
	if (authority.pagesError)
		return { kind: "error", message: authority.pagesError.message };
	if (authority.runtimeStatus.status === "error")
		return {
			kind: "error",
			message:
				authority.runtimeStatus.error?.message ??
				"Playback runtime is unavailable.",
		};
	if (!authority.actionsReady)
		return { kind: "error", message: "Playback controls are unavailable." };
	if (
		!authority.pagesReady ||
		!authority.deskReady ||
		authority.runtimeStatus.status !== "ready" ||
		!authority.projectionsReady
	)
		return { kind: "loading", message: "Loading Playback controls…" };
	return { kind: "ready" };
}
