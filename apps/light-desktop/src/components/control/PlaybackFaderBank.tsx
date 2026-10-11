import { PlaybackArtworkProvider } from "./playbackFaderBank/PlaybackArtworkProvider";
import { ErrorAlert, formatErrorDetails } from "@tosklight/ui";
import { memo, useEffect, useId, useRef, useState } from "react";
import { routeControlSurfaceIntentWithFeedback } from "../../features/controlSurfaceInteraction/registry";
import { AssignedPlaybackConfigurationModal } from "./AssignedPlaybackConfigurationModal";
import {
	type PlaybackFaderBankProps,
	usePlaybackBankController,
} from "./playbackFaderBank/controller";
import { PlaybackBankScrollControls } from "./playbackFaderBank/PlaybackBankScrollControls";
import { SoloFenceOverlay } from "./playbackFaderBank/SoloFenceOverlay";
import { playbackCellKey } from "./playbackFaderBank/projection";
import { scrollablePlaybackGeometry } from "./playbackFaderBank/scrollableGeometry";
import { PlaybackSlot } from "./playbackFaderBank/PlaybackSlot";
import { PlaybackGridSoloControls } from "./playbackFaderBank/PlaybackGridSoloControls";
import { usePlaybackGridSolo } from "./playbackFaderBank/usePlaybackGridSolo";
import { usePlaybackBankConfiguration } from "./playbackFaderBank/usePlaybackBankConfiguration";
import { CueRecordChoiceModal } from "./playbackFaderBank/CueRecordChoiceModal";

export const PlaybackFaderBank = memo<PlaybackFaderBankProps>(
	function PlaybackFaderBank(props: PlaybackFaderBankProps = {}) {
		const controller = usePlaybackBankController(props);
		const [bankElement, setBankElement] = useState<HTMLDivElement | null>(null);
		const viewport = useRef<HTMLDivElement | null>(null);
		const interactionSurfaceId = useId();
		usePlaybackBankConfiguration(controller, interactionSurfaceId);
		const solo = usePlaybackGridSolo(controller);
		const geometry =
			props.scrollable && !controller.hardware
				? scrollablePlaybackGeometry(
						controller.rowTracks,
						controller.columns,
						controller.surfaceId === "main",
					)
				: null;
		// Once the grid has rendered for a ready topology, transient projection refetches
		// (hardware connect, layout changes) must not tear the slot elements down — only a
		// topology-scope reset returns the bank to its loading placeholder.
		const [rendered, setRendered] = useState(false);
		useEffect(() => {
			if (controller.authorityReady) setRendered(true);
			else if (!controller.topology.ready) setRendered(false);
		}, [controller.authorityReady, controller.topology.ready]);
		const showGrid =
			controller.authorityReady || (rendered && controller.topology.ready);
		if (!showGrid)
			return (
				<ErrorAlert
					as="div"
					className="playback-fader-bank playback-authority-status"
					role={controller.authorityError ? "alert" : "status"}
					copyText={
						controller.authorityError
							? formatErrorDetails(controller.authorityError)
							: undefined
					}
				>
					{controller.authorityError?.message ?? "Loading Playbacks…"}
				</ErrorAlert>
			);
		return (
			<section className="playback-surface-controls">
				<div
					className={
						geometry && controller.surfaceId === "main"
							? "playback-main-toolbar"
							: undefined
					}
				>
					<PlaybackGridSoloControls solo={solo} />
					{geometry && controller.surfaceId === "main" ? (
						<PlaybackBankScrollControls viewport={viewport} />
					) : null}
				</div>
				<div
					ref={viewport}
					className={
						geometry ? "playback-bank-viewport" : "playback-bank-fixed"
					}
				>
					{/* biome-ignore lint/a11y/noStaticElementInteractions: This container routes the pointer-only context menu to the exact typed playback target. */}
					<div
						ref={setBankElement}
						onPointerDownCapture={(event) => {
							if (solo.selecting) solo.pointerDown(event);
							else void controller.selectSurface();
						}}
						onFocusCapture={() => {
							if (!solo.selecting) void controller.selectSurface();
						}}
						onPointerUpCapture={solo.block}
						onPointerMoveCapture={solo.block}
						onPointerCancelCapture={solo.block}
						onWheelCapture={solo.block}
						onDoubleClickCapture={solo.block}
						onClickCapture={solo.block}
						onKeyDownCapture={solo.keyDown}
						onKeyUpCapture={solo.block}
						data-playback-surface={controller.surfaceId}
						data-command-surface={
							controller.playbackDesk?.selected_surface_id ===
							controller.surfaceId
						}
						className={`playback-fader-bank ${controller.hardware ? "hardware-layout" : "touch-layout"} ${controller.playbackDesk?.selected_surface_id === controller.surfaceId ? "command-surface" : ""}`}
						onContextMenu={(event) => {
							if (solo.block(event)) return;
							event.preventDefault();
							const card = (event.target as Element).closest<HTMLElement>(
								"[data-playback-slot]",
							);
							const slot = Number(card?.dataset.playbackSlot);
							if (!Number.isInteger(slot)) return;
							routeControlSurfaceIntentWithFeedback({
								type: "configure_playback",
								source: "context_menu",
								surfaceId: interactionSurfaceId,
								slot,
							});
						}}
						style={{
							gridTemplateColumns:
								geometry?.gridTemplateColumns ??
								`repeat(${controller.columns}, minmax(0, 1fr))`,
							gridTemplateRows:
								geometry?.gridTemplateRows ?? controller.rowTracks,
							minWidth: geometry?.minWidth,
							minHeight: geometry?.minHeight,
						}}
					>
						<PlaybackArtworkProvider candidates={controller.slots}>
							{controller.slots.map((slotData) => (
								<PlaybackSlot
									controller={controller}
									slotData={slotData}
									soloClasses={solo.classes(slotData)}
									key={playbackCellKey(slotData)}
								/>
							))}
						</PlaybackArtworkProvider>
						<SoloFenceOverlay
							bank={bankElement}
							page={controller.activePageNumber}
							regions={solo.fenceRegions}
							layoutKey={controller.slots}
						/>
					</div>
				</div>
				{controller.configuration && (
					<AssignedPlaybackConfigurationModal
						{...controller.configuration}
						onClose={() => controller.setConfiguration(null)}
					/>
				)}
				<CueRecordChoiceModal controller={controller} />
			</section>
		);
	},
	equalPlaybackFaderBankProps,
);

function equalPlaybackFaderBankProps(
	left: PlaybackFaderBankProps,
	right: PlaybackFaderBankProps,
) {
	return (
		(left.surfaceId ?? "main") === (right.surfaceId ?? "main") &&
		Boolean(left.scrollable) === Boolean(right.scrollable) &&
		(left.pageNumber ?? null) === (right.pageNumber ?? null) &&
		(left.firstSlot ?? 1) === (right.firstSlot ?? 1) &&
		(left.count ?? null) === (right.count ?? null) &&
		(left.rows ?? null) === (right.rows ?? null) &&
		(left.buttons ?? null) === (right.buttons ?? null) &&
		Boolean(left.hardwareConnected) === Boolean(right.hardwareConnected) &&
		equalPlaybackLayout(left.playbackLayout, right.playbackLayout)
	);
}

function equalPlaybackLayout(
	left: PlaybackFaderBankProps["playbackLayout"],
	right: PlaybackFaderBankProps["playbackLayout"],
) {
	if (left === right) return true;
	if (!left || !right) return false;
	return (
		left.rows.length === right.rows.length &&
		left.rows.every((row, index) => {
			const candidate = right.rows[index];
			if (!candidate) return false;
			return (
				row.first_position === candidate.first_position &&
				row.count === candidate.count &&
				row.presentation === candidate.presentation &&
				row.button_count === candidate.button_count
			);
		})
	);
}

export type { PlaybackFaderBankProps } from "./playbackFaderBank/controller";
export {
	emptyConfiguration,
	playbackButtonLabel,
} from "./playbackFaderBank/feedback";
export { playbackRowUnits } from "./playbackFaderBank/projection";
