import { useRef } from "react";
import { PositionDialog } from "../../components/modals/specialDialogs/intention/PositionDialog";

export interface PositionProgrammingEditorProps {
	fits: boolean;
	pan: number;
	tilt: number;
	onAngles(pan: number, tilt: number): void;
	onGestureEnd?(): void;
	onClose(): void;
}

/** Deterministic demo descriptors for the mockup only; production supplies the fixture's own. */
const DEMO_PAN = { minimum: -720, maximum: 720, step: .1, keyStep: 1, largeKeyStep: 360 } as const;
const DEMO_TILT = { minimum: -135, maximum: 135, step: .1 } as const;
const DEMO_JOYSTICK = { panDegreesPerSecond: 120, tiltDegreesPerSecond: 90 } as const;

/** Mockup adapter: merges per-axis changes into the mockup's angle intent and pages back after edits. */
export function PositionProgrammingEditor({ pan, tilt, onAngles, onGestureEnd, onClose }: PositionProgrammingEditorProps) {
	const latest = useRef({ pan, tilt });
	latest.current = { pan, tilt };
	return <PositionDialog pan={{ ...DEMO_PAN, value: pan }} tilt={{ ...DEMO_TILT, value: tilt }} joystick={DEMO_JOYSTICK}
		onChange={change => {
			latest.current = { pan: change.pan ?? latest.current.pan, tilt: change.tilt ?? latest.current.tilt };
			onAngles(latest.current.pan, latest.current.tilt);
		}}
		onGestureEnd={(_, { changed }) => { if (changed) onGestureEnd?.(); }}
		onGestureCancel={(_, reason, { changed }) => { if (changed && reason !== "teardown") onGestureEnd?.(); }}
		onClose={onClose} />;
}
