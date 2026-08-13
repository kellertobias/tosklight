import { Button } from "@tosklight/ui";
import {
	ProgrammerKeypadView,
	programmerKeyClassName,
} from "@tosklight/ui/command";
import {
	numericPadLayout,
	type SoftwareDeskInputMode,
	softwareDeskKeypadLayout,
	softwareKeyLabel,
} from "@tosklight/ui/programmer-keypad";
import { HighlightControls } from "./HighlightControls";
import { useNumericPadController } from "./numericPad/useNumericPadController";
import { ProgrammerFadeFader } from "./ProgrammerFadeFader";
import type { SoftwareKey } from "./softwareKeypad";

export { numericPadLayout } from "@tosklight/ui/programmer-keypad";

export function softwareDeskInputMode(): SoftwareDeskInputMode {
	if (typeof window === "undefined") return "keyboard";
	return window.matchMedia?.("(pointer: coarse)").matches &&
		(navigator.maxTouchPoints ?? 0) > 0
		? "touch"
		: "keyboard";
}

export function shiftedSoftwareKeyLabel(key: SoftwareKey, shifted: boolean) {
	if (!shifted) return softwareDeskKeyLabel(key);
	return (
		(
			{
				"0": "ALL",
				"1": "INTENSITY",
				"2": "COLOR",
				"3": "POSITION",
				"4": "BEAM",
				"5": "DYNAMICS",
				"6": "SHAPERS",
				"7": "FOCUS",
				"8": "CONTROL",
				"9": "MEDIA",
				CUE: "TIMECODE",
				PLAYBACK: "MACRO",
				ESC: "UNDO",
				ENT: "LOCK",
				CLR: "FREEZE",
				PRE: "PRELOAD GO CLEAR",
				REC: "UPDATE",
				MOV: "COPY",
			} as Partial<Record<SoftwareKey, string>>
		)[key] ?? softwareDeskKeyLabel(key)
	);
}

export function softwareDeskKeyLabel(key: SoftwareKey) {
	return key === "PLAYBACK" ? "PBK" : softwareKeyLabel(key);
}

function softwareDeskKeyPresentation(
	key: SoftwareKey,
	shifted: boolean,
	deskLocked: boolean,
	unfreezeNext: boolean,
) {
	const primary = softwareDeskKeyLabel(key);
	if (!shifted) return { primary, secondary: null };
	const secondary =
		key === "ENT" && deskLocked
			? "UNLOCK"
			: key === "CLR" && unfreezeNext
				? "UNFREEZE"
				: shiftedSoftwareKeyLabel(key, true);
	return {
		primary,
		secondary: secondary === primary ? null : secondary,
	};
}

function SoftwareDeskKeyCaption({
	primary,
	secondary,
}: {
	primary: string;
	secondary: string | null;
}) {
	return (
		<span className="shifted-key-caption">
			<span>{primary}</span>
			{secondary && <small className="shift-action-label">{secondary}</small>}
		</span>
	);
}

export function NumericPad({
	demo = false,
	inputMode,
}: {
	demo?: boolean;
	inputMode?: SoftwareDeskInputMode;
} = {}) {
	const pad = useNumericPadController();
	if (!demo) {
		const mode = inputMode ?? softwareDeskInputMode();
		return (
			<ProgrammerKeypadView
				programmerFade={<ProgrammerFadeFader compact />}
				highlightControls={<HighlightControls />}
				onPress={pad.press}
				layout={softwareDeskKeypadLayout(mode)}
				labelForKey={(key) => (
					<SoftwareDeskKeyCaption
						{...softwareDeskKeyPresentation(
							key,
							pad.state.shiftArmed,
							pad.deskLocked,
							pad.unfreezeNext,
						)}
					/>
				)}
				ariaLabelForKey={(key) => {
					const { primary, secondary } = softwareDeskKeyPresentation(
						key,
						pad.state.shiftArmed,
						pad.deskLocked,
						pad.unfreezeNext,
					);
					return secondary ? `${primary}, Shift: ${secondary}` : primary;
				}}
				clearState={pad.clearState}
				activeKeys={[
					...(pad.state.shiftArmed ? (["SHIFT"] as const) : []),
					...(pad.state.patchSetArmed ||
					pad.state.presetSetArmed ||
					pad.state.cueListSetArmed ||
					pad.state.playbackSetArmed
						? (["SET"] as const)
						: []),
				]}
				classNameForKey={(key) => keyClass(key, pad)}
			/>
		);
	}
	return (
		<div className="numeric-pad programmer-number-block demo-number-block">
			<div className="numeric-pad-section numeric-pad-command-section">
				<DemoActions pad={pad} />
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
				className={`demo-preload ${pad.preload.armed ? "preload-go" : ""}`}
				disabled={!pad.preload.ready}
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
					className={`${programmerKeyClassName(key, pad.clearState)} ${keyClass(key, pad)}`}
					key={key}
				>
					{softwareKeyLabel(key)}
				</Button>
			);
		});
}

function keyClass(key: SoftwareKey, pad: NumericPadController) {
	const shifted = key === "SHIFT" && pad.state.shiftArmed ? "shift-armed" : "";
	const setArmed =
		key === "SET" &&
		((pad.state.builtIn === "patch" && pad.state.patchSetArmed) ||
			pad.state.presetSetArmed ||
			pad.state.cueListSetArmed ||
			pad.state.playbackSetArmed)
			? "patch-set-armed"
			: "";
	return `${shifted} ${setArmed}`;
}
