import type { SoftwareKey } from "@tosklight/ui/programmer-keypad";
import { type CSSProperties, useRef, useState } from "react";
import { darkLamp, type Lamp } from "../controller/types";

type HardwareKey =
	| SoftwareKey
	| "HIGH"
	| "PREV"
	| "NEXT"
	| "ALL"
	| "RECORD"
	| "PRELOAD GO"
	| "PROGRAMMER / PLAYBACK";

interface ControlButtonProps {
	label: string;
	lamp?: Lamp;
	onDown: () => void;
	onUp: () => void;
	className?: string;
	style?: CSSProperties;
	keypadKey?: HardwareKey;
	showHoldFeedback?: boolean;
	disabled?: boolean;
}

export function ControlButton({
	label,
	lamp = darkLamp,
	onDown,
	onUp,
	className = "",
	style,
	keypadKey,
	showHoldFeedback = true,
	disabled = false,
}: ControlButtonProps) {
	const timer = useRef<number | undefined>(undefined);
	const [long, setLong] = useState(false);
	const [pressed, setPressed] = useState(false);

	const release = () => {
		clearTimeout(timer.current);
		if (!disabled) onUp();
		window.setTimeout(() => setPressed(false), 90);
	};

	return (
		<button
			type="button"
			disabled={disabled}
			className={`control-button ${lamp.state} ${lamp.state === "on" && lamp.bpm ? "beat" : ""} ${pressed ? "local-pressed" : ""} ${className}`}
			data-keypad-key={keypadKey}
			style={
				{
					...style,
					"--lamp": lamp.color,
					"--bpm": lamp.bpm ?? 60,
				} as CSSProperties
			}
			onPointerDown={(event) => {
				if (disabled) return;
				event.currentTarget.setPointerCapture(event.pointerId);
				setPressed(true);
				setLong(false);
				timer.current = window.setTimeout(() => setLong(true), 650);
				onDown();
			}}
			onPointerUp={release}
			onPointerCancel={release}
		>
			<span>{label}</span>
			{showHoldFeedback && long && <i>LONG</i>}
		</button>
	);
}
