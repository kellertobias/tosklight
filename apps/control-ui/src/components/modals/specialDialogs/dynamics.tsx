import { VerticalTouchFader } from "../../control/VerticalTouchFader";

interface DynamicsDialogProps {
	speed: number;
	disabled: boolean;
	apply: (attribute: string, value: number) => Promise<void>;
	setSpeed: (speed: number) => void;
}

export function DynamicsDialog({
	speed,
	disabled,
	apply,
	setSpeed,
}: DynamicsDialogProps) {
	return (
		<VerticalTouchFader
			label="Dynamic speed"
			value={speed}
			disabled={disabled}
			maximum={240}
			display={`${speed} BPM`}
			onChange={(value) => {
				setSpeed(value);
				void apply("dynamic.speed", value / 240);
			}}
		/>
	);
}
