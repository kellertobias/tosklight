import {
	attachedHighlightKeys,
	attachedKeypadContentRowOffset,
	attachedProgrammerActionLayout,
	controlSurfaceOscPaths,
	type ProgrammerControlAction,
} from "@tosklight/ui/control-surface-contracts";
import {
	type NumericPadSection,
	numericPadLayout,
	oscProgrammerActionForKey,
	softwareKeyLabel,
} from "@tosklight/ui/programmer-keypad";
import { useRef } from "react";
import { ControlButton } from "../components/ControlButton";
import { actionRequestId } from "../controller/actionRequestId";
import {
	darkLamp,
	type HighlightFeedback,
	type Lamp,
	type SendControl,
} from "../controller/types";
import { TimeFader } from "./programmer/TimeFader";

interface ProgrammerSurfaceProps {
	updateArmed: boolean;
	lamps: Record<string, Lamp>;
	highlight: HighlightFeedback;
	send: SendControl;
}

export function ProgrammerSurface({
	updateArmed,
	lamps,
	highlight,
	send,
}: ProgrammerSurfaceProps) {
	const actionIds = useRef(new Map<ProgrammerControlAction, string>());
	const action = (name: ProgrammerControlAction, down: boolean) => {
		const requestId = down
			? actionRequestId()
			: (actionIds.current.get(name) ?? actionRequestId());
		if (down) actionIds.current.set(name, requestId);
		else actionIds.current.delete(name);
		send(controlSurfaceOscPaths.programmer(name), [down, requestId]);
	};

	const renderKeypadSection = (section: NumericPadSection) =>
		numericPadLayout
			.filter((item) => item.section === section)
			.map(({ key, column, row, rowSpan = 1 }) => {
				const sectionColumn = section === "commands" ? column : column - 3;
				const displayRow = row + attachedKeypadContentRowOffset;
				const actionName = oscProgrammerActionForKey(key);
				return (
					<ControlButton
						key={key}
						keypadKey={key}
						className={`key-${actionName} ${key === "ENT" ? "key-enter" : ""}`}
						label={softwareKeyLabel(key)}
						style={{
							gridColumn: sectionColumn,
							gridRow: `${displayRow} / span ${rowSpan}`,
						}}
						onDown={() => action(actionName, true)}
						onUp={() => action(actionName, false)}
					/>
				);
			});

	return (
		<aside className="programmer-panel">
			<div className="hardware-number-block">
				<div className="hardware-keypad-section hardware-keypad-command-section">
					<ControlButton
						keypadKey="PROGRAMMER / PLAYBACK"
						className="key-prog-playback"
						label="PROGRAMMER / PLAYBACK"
						style={{ gridColumn: "1 / span 4", gridRow: 2 }}
						onDown={() => action("prog-playback", true)}
						onUp={() => action("prog-playback", false)}
					/>
					<ControlButton
						keypadKey="RECORD"
						className="key-record"
						label={updateArmed ? "UPDATE" : "RECORD"}
						lamp={updateArmed ? { color: "#f4b942", state: "on" } : darkLamp}
						style={{
							gridColumn: `${attachedProgrammerActionLayout.record.column} / span ${attachedProgrammerActionLayout.record.columnSpan}`,
							gridRow: `${attachedProgrammerActionLayout.record.row} / span ${attachedProgrammerActionLayout.record.rowSpan}`,
						}}
						onDown={() => action("record", true)}
						onUp={() => action("record", false)}
					/>
					<ControlButton
						keypadKey="PRELOAD GO"
						className="key-preload-go"
						label="PRELOAD GO"
						style={{
							gridColumn: `${attachedProgrammerActionLayout.preload.column} / span ${attachedProgrammerActionLayout.preload.columnSpan}`,
							gridRow: `${attachedProgrammerActionLayout.preload.row} / span ${attachedProgrammerActionLayout.preload.rowSpan}`,
						}}
						onDown={() => action("preload", true)}
						onUp={() => action("preload", false)}
					/>
					{renderKeypadSection("commands")}
				</div>
				<div className="hardware-keypad-section hardware-keypad-number-section">
					{attachedHighlightKeys.map((item) => (
						<ControlButton
							key={item.action}
							className={`highlight-key ${item.action === "toggle" ? "highlight-high" : `highlight-${item.action}`}`}
							label={item.label}
							lamp={highlightLamp(item.action, highlight, lamps.highlight)}
							keypadKey={item.label}
							showHoldFeedback={item.action !== "toggle"}
							disabled={!highlightActionEnabled(item.action, highlight)}
							style={{ gridColumn: item.column, gridRow: item.row }}
							onDown={() =>
								send(controlSurfaceOscPaths.highlight(item.action), [true])
							}
							onUp={() =>
								send(controlSurfaceOscPaths.highlight(item.action), [false])
							}
						/>
					))}
					{renderKeypadSection("numbers")}
				</div>
			</div>
			<div className="fade-times">
				<TimeFader
					label="Prog Fade"
					path={controlSurfaceOscPaths.programmerFade("programmer")}
					maximum={20}
					send={send}
				/>
				<TimeFader
					label="Cue Fade"
					path={controlSurfaceOscPaths.programmerFade("cue")}
					maximum={60}
					send={send}
				/>
			</div>
		</aside>
	);
}

function highlightLamp(
	action: (typeof attachedHighlightKeys)[number]["action"],
	highlight: HighlightFeedback,
	toggleLamp: Lamp | undefined,
): Lamp {
	if (action === "toggle") return toggleLamp ?? darkLamp;
	if (action === "previous" && highlight.active && highlight.canPrevious) {
		return { color: "#68b9c7", state: "on" };
	}
	if (action === "next" && highlight.active && highlight.canNext) {
		return { color: "#68b9c7", state: "on" };
	}
	return darkLamp;
}

function highlightActionEnabled(
	action: (typeof attachedHighlightKeys)[number]["action"],
	highlight: HighlightFeedback,
): boolean {
	if (action === "toggle") return true;
	if (!highlight.active) return false;
	if (action === "previous") return highlight.canPrevious;
	if (action === "next") return highlight.canNext;
	return true;
}
