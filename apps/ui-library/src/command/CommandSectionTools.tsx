import {
	type ComponentProps,
	type CSSProperties,
	type ReactNode,
	useRef,
} from "react";
import { Button } from "../common";
import {
	type NumericPadLayoutItem,
	numericPadLayout,
	type SoftwareKey,
	softwareKeyLabel,
} from "../programmerKeypad";

export type ProgrammerClearState = "idle" | "selection" | "active-values";

export interface ProgrammerKeypadViewProps {
	programmerFade: ReactNode;
	highlightControls: ReactNode;
	onPress: (key: SoftwareKey) => void;
	onHold?: (key: SoftwareKey) => void;
	holdKeys?: readonly SoftwareKey[];
	activeKeys?: readonly SoftwareKey[];
	clearState?: ProgrammerClearState;
	disabledKeys?: readonly SoftwareKey[];
	classNameForKey?: (key: SoftwareKey) => string;
	layout?: readonly NumericPadLayoutItem[];
	labelForKey?: (key: SoftwareKey) => ReactNode;
	ariaLabelForKey?: (key: SoftwareKey) => string;
}

const ACTION_KEYS = new Set<SoftwareKey>([
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
]);

export function programmerKeyClassName(
	key: SoftwareKey,
	clearState: ProgrammerClearState,
) {
	const action = ACTION_KEYS.has(key) ? "action" : "";
	const enter = key === "ENT" ? "enter" : "";
	const clear =
		key === "CLR"
			? `clear ${
					clearState === "selection"
						? "clear-active"
						: clearState === "active-values"
							? "clear-warning"
							: "clear-idle"
				}`
			: "";
	return `${action} ${enter} ${clear}`.trim();
}

export function ProgrammerKeypadView({
	programmerFade,
	highlightControls,
	onPress,
	onHold,
	holdKeys = [],
	activeKeys = [],
	clearState = "idle",
	disabledKeys = [],
	classNameForKey,
	layout = numericPadLayout,
	labelForKey = softwareKeyLabel,
	ariaLabelForKey,
}: ProgrammerKeypadViewProps) {
	const renderKeys = (section: "commands" | "numbers") =>
		layout
			.filter((item) => item.section === section)
			.map(({ key, column, row, rowSpan = 1 }) => {
				const sectionColumn = section === "commands" ? column : column - 3;
				const displayRow = row + 1;
				const active = activeKeys.includes(key);
				const disabled = disabledKeys.includes(key);
				return (
					<ProgrammerKeyButton
						aria-label={ariaLabelForKey?.(key)}
						aria-pressed={active || undefined}
						className={`${programmerKeyClassName(key, clearState)} ${classNameForKey?.(key) ?? ""} ${active ? "active" : ""}`.trim()}
						data-keypad-key={key}
						disabled={disabled}
						key={key}
						onPress={() => onPress(key)}
						onHold={holdKeys.includes(key) ? () => onHold?.(key) : undefined}
						style={{
							gridColumn: sectionColumn,
							gridRow: `${displayRow} / span ${rowSpan}`,
						}}
					>
						{labelForKey(key)}
					</ProgrammerKeyButton>
				);
			});
	return (
		<div className="numeric-pad programmer-number-block">
			<div className="numeric-pad-section numeric-pad-command-section">
				<div
					className="numeric-pad-fade"
					data-grid-column-span="2"
					data-grid-row-span="2"
					style={{ gridColumn: "1 / span 2", gridRow: "1 / span 2" }}
				>
					{programmerFade}
				</div>
				{renderKeys("commands")}
			</div>
			<div className="numeric-pad-section numeric-pad-number-section">
				{highlightControls}
				{renderKeys("numbers")}
			</div>
		</div>
	);
}

function ProgrammerKeyButton({
	onPress,
	onHold,
	children,
	...props
}: ComponentProps<typeof Button> & {
	onPress: () => void;
	onHold?: () => void;
}) {
	const timer = useRef<number | null>(null);
	const held = useRef(false);
	const cancel = () => {
		if (timer.current != null) window.clearTimeout(timer.current);
		timer.current = null;
	};
	return (
		<Button
			{...props}
			onPointerDown={() => {
				held.current = false;
				if (!onHold) return;
				timer.current = window.setTimeout(() => {
					timer.current = null;
					held.current = true;
					onHold();
				}, 650);
			}}
			onPointerUp={cancel}
			onPointerCancel={cancel}
			onPointerLeave={cancel}
			onClick={() => {
				cancel();
				if (!held.current) onPress();
				held.current = false;
			}}
		>
			{children}
		</Button>
	);
}

export interface SpeedGroupViewModel {
	id: "A" | "B" | "C" | "D" | "E";
	bpm?: number;
	display: string;
	active?: boolean;
	soundEnabled?: boolean;
}

export interface PlaybackToolsViewProps {
	pageControls: ReactNode;
	programmerFade: ReactNode;
	cueFade: ReactNode;
	releaseFade: ReactNode;
	speedGroups: readonly SpeedGroupViewModel[];
	setArmed?: boolean;
	shiftArmed?: boolean;
	onCommandKey: (key: "SET" | "CPY" | "MOV" | "DEL" | "SHIFT") => void;
	onSpeedPointerDown?: (
		group: SpeedGroupViewModel["id"],
		event: React.PointerEvent<HTMLButtonElement>,
	) => void;
	onSpeedPointerEnd?: () => void;
	onSpeedActivate?: (
		group: SpeedGroupViewModel["id"],
		event: React.MouseEvent<HTMLButtonElement>,
	) => void;
	onSpeedSettings?: (group: SpeedGroupViewModel["id"]) => void;
	overlays?: ReactNode;
}

export function PlaybackToolsView({
	pageControls,
	programmerFade,
	cueFade,
	releaseFade,
	speedGroups,
	setArmed = false,
	shiftArmed = false,
	onCommandKey,
	onSpeedPointerDown,
	onSpeedPointerEnd,
	onSpeedActivate,
	onSpeedSettings,
	overlays,
}: PlaybackToolsViewProps) {
	return (
		<div className="playback-tools">
			<div className="playback-command-keys">
				{(["SET", "CPY", "MOV", "DEL", "SHIFT"] as const).map((key) => (
					<Button
						aria-pressed={
							(key === "SET" && setArmed) ||
							(key === "SHIFT" && shiftArmed) ||
							undefined
						}
						className={
							(key === "SET" && setArmed) || (key === "SHIFT" && shiftArmed)
								? "active"
								: ""
						}
						data-keypad-key={key}
						key={key}
						onClick={() => onCommandKey(key)}
					>
						{key}
					</Button>
				))}
			</div>
			{pageControls}
			{programmerFade}
			<div className="cue-fade-master">{cueFade}</div>
			<div className="release-fade-master">{releaseFade}</div>
			<div className="speed-group-stack">
				{speedGroups.map((group) => (
					<Button
						aria-label={
							group.bpm === undefined
								? `Speed group ${group.id}, loading`
								: `Speed group ${group.id}, ${group.display} BPM`
						}
						className={`${group.active === false ? "" : "active"} ${group.soundEnabled ? "sound-enabled" : ""}`.trim()}
						key={group.id}
						onClick={(event) => onSpeedActivate?.(group.id, event)}
						onContextMenu={(event) => {
							event.preventDefault();
							onSpeedSettings?.(group.id);
						}}
						onPointerCancel={onSpeedPointerEnd}
						onPointerDown={(event) => onSpeedPointerDown?.(group.id, event)}
						onPointerLeave={onSpeedPointerEnd}
						onPointerUp={onSpeedPointerEnd}
						style={
							group.bpm === undefined
								? undefined
								: ({ "--bpm": group.bpm } as CSSProperties)
						}
					>
						<strong className="speed-group-label">{group.id}</strong>
						<span className="speed-group-value">{group.display}</span>
						<small className="speed-group-unit">BPM</small>
					</Button>
				))}
			</div>
			{overlays}
		</div>
	);
}

export interface HardwareControlValue {
	id: "programmer-fade" | "cue-fade" | "release-fade" | "page";
	label: string;
	display: string;
	disabled?: boolean;
	ariaLabel?: string;
	settings?: boolean;
}

export interface HardwareControlSummaryViewProps {
	values: readonly HardwareControlValue[];
	speedGroups: readonly SpeedGroupViewModel[];
	onValue: (id: HardwareControlValue["id"]) => void;
	onValueSettings?: (id: HardwareControlValue["id"]) => void;
	onSpeedPointerDown?: (
		group: SpeedGroupViewModel["id"],
		event: React.PointerEvent<HTMLButtonElement>,
	) => void;
	onSpeedPointerEnd?: () => void;
	onSpeedActivate?: (
		group: SpeedGroupViewModel["id"],
		event: React.MouseEvent<HTMLButtonElement>,
	) => void;
	onSpeedSettings?: (group: SpeedGroupViewModel["id"]) => void;
	overlays?: ReactNode;
}

export function HardwareControlSummaryView({
	values,
	speedGroups,
	onValue,
	onValueSettings,
	onSpeedPointerDown,
	onSpeedPointerEnd,
	onSpeedActivate,
	onSpeedSettings,
	overlays,
}: HardwareControlSummaryViewProps) {
	return (
		<div className="hardware-control-summary">
			<div className="hardware-values">
				{values.map((value) => (
					<Button
						aria-label={value.ariaLabel}
						disabled={value.disabled}
						key={value.id}
						onClick={() => onValue(value.id)}
						onContextMenu={
							value.settings && onValueSettings
								? (event) => {
										event.preventDefault();
										onValueSettings(value.id);
									}
								: undefined
						}
					>
						<small>{value.label}</small>
						<b>{value.display}</b>
					</Button>
				))}
			</div>
			<div className="hardware-speed-groups">
				{speedGroups.map((group) => (
					<Button
						aria-label={
							group.bpm === undefined
								? `Speed group ${group.id}, loading`
								: `Speed group ${group.id}, ${group.display} BPM`
						}
						key={group.id}
						onClick={(event) => onSpeedActivate?.(group.id, event)}
						onContextMenu={(event) => {
							event.preventDefault();
							onSpeedSettings?.(group.id);
						}}
						onPointerCancel={onSpeedPointerEnd}
						onPointerDown={(event) => onSpeedPointerDown?.(group.id, event)}
						onPointerLeave={onSpeedPointerEnd}
						onPointerUp={onSpeedPointerEnd}
						style={
							group.bpm === undefined
								? undefined
								: ({ "--bpm": group.bpm } as CSSProperties)
						}
					>
						<b>{group.id}</b>
						<span>{group.display} BPM</span>
					</Button>
				))}
			</div>
			{overlays}
		</div>
	);
}
