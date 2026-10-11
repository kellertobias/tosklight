import type { PlaybackAddressValue } from "../../../api/playbackAddress";
import { Button, Input } from "@tosklight/ui";
import type { PlaybackButtonAction } from "../../../api/types";
import type { PlaybackBankController } from "./controller";
import { isPlaybackSetClickArmed } from "./actions";
import { isHeldAction, playbackButtonLabel } from "./feedback";
import type { PlaybackSlotProjection } from "./types";

export function ExpandedPlaybackControls({
	controller,
	slotData,
}: {
	controller: PlaybackBankController;
	slotData: PlaybackSlotProjection;
}) {
	const { playback, footprint, row } = slotData;
	if (!playback || !footprint || footprint.effective === "normal") return null;
	const stored = playback.footprint;
	if (!stored || stored.type !== footprint.effective) return null;
	const visibleButtonCount = Math.min(
		row?.button_count ?? 3,
		playback.button_count ?? 3,
	);
	const buttons =
		stored.type === "taller"
			? [stored.upper_button]
			: stored.right_buttons.slice(0, visibleButtonCount);
	return (
		<fieldset
			className={`expanded-playback-controls ${stored.type}`}
			data-playback-footprint={stored.type}
			aria-label={`${stored.type === "taller" ? "Upper" : "Right"} controls for Playback ${slotData.slot}`}
		>
			{buttons.map((action, index) => (
				<ExpandedButton
					action={action}
					button={index + 4}
					controller={controller}
					address={playback.address}
					key={`${index}-${action}`}
				/>
			))}
			{stored.type === "wider" && (
				<label className="expanded-playback-fader">
					<span>Right fader · {stored.right_fader.replaceAll("_", " ")}</span>
					<Input
						type="range"
						min={0}
						max={100}
						defaultValue={0}
						disabled={!controller.runtimeActions}
						onChange={(event) =>
							void controller.runtimeActions?.playbackAction(
								playback.address,
								"configured-fader-absolute",
								{
									fader: 2,
									value: Number(event.currentTarget.value) / 100,
									control_source: {
										surface_id: controller.controlSourceId,
										control_id: 8,
									},
								},
							)
						}
					/>
				</label>
			)}
		</fieldset>
	);
}

function ExpandedButton({
	controller,
	address,
	button,
	action,
}: {
	controller: PlaybackBankController;
	address: PlaybackAddressValue;
	button: number;
	action: PlaybackButtonAction;
}) {
	const send = (pressed: boolean) =>
		void controller.runtimeActions?.playbackAction(address, "button", {
			button,
			pressed,
			control_source: {
				surface_id: controller.controlSourceId,
				control_id: button,
			},
		});
	return (
		<Button
			type="button"
			data-playback-button-index={button}
			disabled={!controller.runtimeActions || action === "none"}
			onClick={() => !isHeldAction(action) && send(true)}
			onPointerDown={(event) => {
				if (!isHeldAction(action)) return;
				event.currentTarget.setPointerCapture?.(event.pointerId);
				controller.heldActions.press(
					controller.controlSourceId,
					address,
					button,
					action,
					{ pointerId: event.pointerId },
				);
			}}
			onPointerUp={(event) =>
				controller.heldActions.releaseGesture({ pointerId: event.pointerId })
			}
			onPointerCancel={(event) =>
				controller.heldActions.releaseGesture({ pointerId: event.pointerId })
			}
			onKeyDown={(event) => {
				if (
					controller.state.storeArmed ||
					controller.state.updateArmed ||
					controller.setInteractionArmed ||
					controller.groupAssignmentPending ||
					isPlaybackSetClickArmed(controller) ||
					!isHeldAction(action) ||
					![" ", "Enter"].includes(event.key)
				)
					return;
				event.preventDefault();
				if (!event.repeat)
					controller.heldActions.press(
						controller.controlSourceId,
						address,
						button,
						action,
						{ key: event.key },
					);
			}}
			onKeyUp={(event) =>
				controller.heldActions.releaseGesture({ key: event.key })
			}
		>
			{playbackButtonLabel(action)}
		</Button>
	);
}
