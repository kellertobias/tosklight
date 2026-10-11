import type { CSSProperties } from "react";
import { TouchEncoder } from "../encoders/TouchEncoder";
import type { VerticalTouchFaderAction } from "../faders";

/** Playback rotation changes the level; its center retains the captured action. */
export function PlaybackRotaryControl({
	value,
	label,
	display,
	disabled,
	action,
	accentColor,
	onChange,
}: {
	value: number;
	label: string;
	display: string;
	disabled?: boolean;
	action?: VerticalTouchFaderAction;
	accentColor?: string;
	onChange?: (value: number) => void;
}) {
	const position = Number.isFinite(value)
		? Math.min(100, Math.max(0, value))
		: 0;
	const percentage = `${Number(position.toFixed(1))}%`;
	return (
		<div
			className="playback-rotary-control"
			data-position={position}
			style={
				{ "--playback-rotary-angle": `${position * 2.7}deg` } as CSSProperties
			}
		>
			<TouchEncoder
				centerAction={action ?? { id: "none", label: "None", disabled: true }}
				label={label}
				value={position / 100}
				display={percentage}
				disabled={disabled}
				accentColor={accentColor}
				onSet={(next) => onChange?.(next * 100)}
				onStep={(delta) =>
					onChange?.(Math.min(100, Math.max(0, position + delta * 100)))
				}
			/>
			{display !== percentage && (
				<span className="playback-rotary-feedback">{display}</span>
			)}
		</div>
	);
}
