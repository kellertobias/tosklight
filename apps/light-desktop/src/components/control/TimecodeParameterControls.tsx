import { Button } from "@tosklight/ui";
import { TouchEncoder } from "@tosklight/ui/encoders";
import { useEffect, useState } from "react";
import type { TimecodeEncoderDeck } from "../../features/timecode/timecodeEncoderBridge";
import { HardwareEncoderDisplay } from "./HardwareEncoderDisplay";

export function TimecodeParameterControls({
	hardwareConnected,
	deck,
}: {
	hardwareConnected: boolean;
	deck: TimecodeEncoderDeck;
}) {
	const [group, setGroup] = useState<"timeline" | "keyframe">("timeline");
	const slots = deck[group];

	useEffect(() => {
		if (!hardwareConnected) return;
		const handleEncoder = (event: Event) => {
			const detail = (
				event as CustomEvent<{ control: string; value?: string }>
			).detail;
			const slot = slots[Number(detail.control.split("/")[1]) - 1];
			if (!slot || slot.disabled) return;
			const direction =
				detail.value === "up" || detail.value === "right"
					? 1
					: detail.value === "down" || detail.value === "left"
						? -1
						: 0;
			if (!direction) return;
			const step =
				detail.value === "left" || detail.value === "right"
					? slot.coarseStep
					: slot.fineStep;
			slot.set(slot.value + direction * step);
		};
		window.addEventListener("light:encoder-action", handleEncoder);
		return () =>
			window.removeEventListener("light:encoder-action", handleEncoder);
	}, [deck, hardwareConnected, slots]);

	return (
		<div className="parameter-controls timecode-parameter-controls">
			<div className="family-tabs">
				<Button active={group === "timeline"} onClick={() => setGroup("timeline")}>
					Timecode Timeline
				</Button>
				<Button
					active={group === "keyframe"}
					onClick={() => setGroup("keyframe")}
				>
					Selected Keyframe
				</Button>
			</div>
			<div className="parameter-surfaces">
				{slots.map((slot, index) =>
					hardwareConnected ? (
						<HardwareEncoderDisplay
							key={slot.id}
							slot={index + 1}
							activateOnHardwarePress
							target={{
								label: slot.label,
								value: slot.display,
								role: "Turn · Press-turn coarse",
							}}
							editValue={slot.value}
							onEdit={slot.set}
						/>
					) : (
						<TouchEncoder
							key={slot.id}
							label={`Enc ${index + 1} · ${slot.label}`}
							slot={index + 1}
							attributeLabel={slot.label}
							value={slot.value}
							display={slot.display}
							minimum={slot.minimum}
							maximum={slot.maximum}
							slowStep={slot.fineStep}
							fastStep={slot.coarseStep}
							disabled={slot.disabled}
							onStep={(delta) => slot.set(slot.value + delta)}
							onSet={slot.set}
						/>
					),
				)}
				{hardwareConnected &&
					Array.from({ length: Math.max(0, 6 - slots.length) }, (_, index) => (
						<HardwareEncoderDisplay
							key={`empty-${index}`}
							slot={slots.length + index + 1}
						/>
					))}
			</div>
		</div>
	);
}
