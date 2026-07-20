import {
	numericPadLayout,
	softwareKeyLabel,
} from "../../../../shared/programmerKeypad";
import type { SoftwareKey } from "./softwareKeypad";
import { Button } from "../common";
import { HighlightControls } from "./HighlightControls";
import { useNumericPadController } from "./numericPad/useNumericPadController";
import { ProgrammerFadeFader } from "./ProgrammerFadeFader";

export { numericPadLayout } from "../../../../shared/programmerKeypad";

const ACTION_KEYS: readonly SoftwareKey[] = [
	"AT",
	"TRU",
	"GRP",
	"SET",
	"DIV",
	"CUE",
	"UND",
	"DEL",
	"MOV",
	"CPY",
	"+",
	"-",
	"TIME",
	"SHIFT",
	"CLR",
];

export function NumericPad({ demo = false }: { demo?: boolean } = {}) {
	const pad = useNumericPadController();
	return (
		<div
			className={`numeric-pad programmer-number-block ${demo ? "demo-number-block" : ""}`}
		>
			<div className="numeric-pad-section numeric-pad-command-section">
				{demo ? (
					<DemoActions pad={pad} />
				) : (
					<div
						className="numeric-pad-fade"
						data-grid-column-span="2"
						data-grid-row-span="2"
						style={{ gridColumn: "1 / span 2", gridRow: "1 / span 2" }}
					>
						<ProgrammerFadeFader compact />
					</div>
				)}
				<NumericKeys section="commands" pad={pad} />
			</div>
			<div className="numeric-pad-section numeric-pad-number-section">
				<HighlightControls />
				<NumericKeys section="numbers" pad={pad} />
			</div>
		</div>
	);
}

type NumericPadController = ReturnType<typeof useNumericPadController>;

function DemoActions({ pad }: { pad: NumericPadController }) {
	return (
		<>
			<Button
				className={`demo-record ${pad.state.storeArmed ? "armed" : ""}`}
				aria-pressed={pad.state.storeArmed}
				style={{ gridColumn: 1, gridRow: 1 }}
				onClick={pad.toggleRecord}
			>
				{pad.state.updateArmed ? "UPDATE" : "RECORD"}
			</Button>
			<Button
				className={`demo-preload ${pad.state.preload === "blind" ? "preload-go" : ""}`}
				style={{ gridColumn: 2, gridRow: 1 }}
				onClick={() => void pad.advancePreload()}
			>
				PRELOAD GO
			</Button>
			<Button
				className="demo-escape"
				style={{ gridColumn: 2, gridRow: 2 }}
				onClick={pad.escape}
			>
				ESCAPE
			</Button>
		</>
	);
}

function NumericKeys({
	section,
	pad,
}: {
	section: "commands" | "numbers";
	pad: NumericPadController;
}) {
	return numericPadLayout
		.filter((item) => item.section === section)
		.map(({ key, column, row, rowSpan = 1 }) => {
			const sectionColumn = section === "commands" ? column : column - 3;
			const displayRow = row + 1;
			return (
				<Button
					onClick={() => pad.press(key)}
					data-keypad-key={key}
					data-grid-column={sectionColumn}
					data-grid-row={displayRow}
					style={{
						gridColumn: sectionColumn,
						gridRow: `${displayRow} / span ${rowSpan}`,
					}}
					className={keyClass(key, pad)}
					key={key}
				>
					{softwareKeyLabel(key)}
				</Button>
			);
		});
}

function keyClass(key: SoftwareKey, pad: NumericPadController) {
	const action = ACTION_KEYS.includes(key)
		? "action"
		: key === "ENT"
			? "enter"
			: "";
	const shifted = key === "SHIFT" && pad.state.shiftArmed ? "shift-armed" : "";
	const setArmed =
		key === "SET" &&
		((pad.state.builtIn === "patch" && pad.state.patchSetArmed) ||
			pad.state.presetSetArmed ||
			pad.state.cueListSetArmed ||
			pad.state.playbackSetArmed)
			? "patch-set-armed"
			: "";
	const clear = key === "CLR" ? `clear ${pad.clearClass}` : "";
	return `${action} ${shifted} ${setArmed} ${clear}`;
}
