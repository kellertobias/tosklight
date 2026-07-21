import { useRef } from "react";
import { useApp } from "../../../state/AppContext";
import { Button } from "../../common";

export function CommandRecordPreload({
	hasRecordableContent,
	pendingSummary,
	preloadLabel,
	preloadArmed,
	preloadActive,
	preloadReady,
	onRecordStart,
	onRecordEnd,
	onRecordCancel,
	onRecordComplete,
	onAdvancePreload,
	onReleasePreload,
}: {
	hasRecordableContent: boolean;
	pendingSummary: string;
	preloadLabel: string;
	preloadArmed: boolean;
	preloadActive: boolean;
	preloadReady: boolean;
	onRecordStart: (shifted: boolean) => void;
	onRecordEnd: () => void;
	onRecordCancel: () => void;
	onRecordComplete: (shifted: boolean) => void;
	onAdvancePreload: () => Promise<void>;
	onReleasePreload: () => Promise<void>;
}) {
	const { state } = useApp();
	const preloadHold = useRef<number | null>(null);
	const preloadHeld = useRef(false);
	const cancelPreloadHold = () => {
		if (preloadHold.current !== null) window.clearTimeout(preloadHold.current);
		preloadHold.current = null;
	};
	return (
		<div className="command-record-preload">
			<Button
				className={`global-store-button ${state.updateArmed ? "update-armed" : state.storeArmed ? "armed" : hasRecordableContent ? "record-ready" : "record-empty"}`}
				aria-pressed={state.updateArmed || state.storeArmed}
				title="REC · Shift+REC: Update · hold Shift+REC: Update Settings"
				onPointerDown={(event) =>
					onRecordStart(state.shiftArmed || event.shiftKey)
				}
				onPointerUp={onRecordEnd}
				onPointerCancel={onRecordCancel}
				onClick={(event) =>
					onRecordComplete(state.shiftArmed || event.shiftKey)
				}
			>
				{state.updateArmed
					? "UPDATE ARMED"
					: state.storeArmed
						? "REC ARMED"
						: "REC"}
			</Button>
			<Button
				className={`preload-button ${preloadArmed ? "preload-go" : "preload-enter"}`}
				disabled={!preloadReady}
				aria-busy={!preloadReady}
				title={
					preloadArmed && pendingSummary
						? `Pending Preload: ${pendingSummary}`
						: preloadActive
							? "Hold to release the active preload scene"
							: undefined
				}
				onPointerDown={() => {
					preloadHeld.current = false;
					if (!preloadActive) return;
					preloadHold.current = window.setTimeout(() => {
						preloadHeld.current = true;
						void onReleasePreload();
					}, 650);
				}}
				onPointerUp={cancelPreloadHold}
				onPointerCancel={cancelPreloadHold}
				onContextMenu={(event) => event.preventDefault()}
				onClick={() => {
					if (!preloadHeld.current) void onAdvancePreload();
					preloadHeld.current = false;
				}}
			>
				<b>{preloadLabel}</b>
				{preloadArmed && pendingSummary ? (
					<small>{pendingSummary}</small>
				) : (
					preloadActive && <small>(Hold: release)</small>
				)}
			</Button>
		</div>
	);
}
