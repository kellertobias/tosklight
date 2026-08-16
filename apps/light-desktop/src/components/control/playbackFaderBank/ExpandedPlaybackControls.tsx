import { Button, Input } from "@tosklight/ui";
import type { PlaybackButtonAction } from "../../../api/types";
import type { PlaybackBankController } from "./controller";
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
					playbackNumber={playback.number}
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
							void controller.runtimeActions?.poolPlaybackAction(
								playback.number,
								"configured-fader",
								{
									fader: 2,
									value: Number(event.currentTarget.value) / 100,
									surface: "physical",
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
	playbackNumber,
	button,
	action,
}: {
	controller: PlaybackBankController;
	playbackNumber: number;
	button: number;
	action: PlaybackButtonAction;
}) {
	const send = (pressed: boolean) =>
		void controller.runtimeActions?.poolPlaybackAction(
			playbackNumber,
			"button",
			{ button, pressed, surface: "physical" },
		);
	return (
		<Button
			type="button"
			data-playback-button-index={button}
			disabled={!controller.runtimeActions || action === "none"}
			onClick={() => !isHeldAction(action) && send(true)}
			onPointerDown={(event) => {
				if (!isHeldAction(action)) return;
				event.currentTarget.setPointerCapture?.(event.pointerId);
				send(true);
			}}
			onPointerUp={() => isHeldAction(action) && send(false)}
			onPointerCancel={() => isHeldAction(action) && send(false)}
			onLostPointerCapture={() => isHeldAction(action) && send(false)}
		>
			{playbackButtonLabel(action)}
		</Button>
	);
}
