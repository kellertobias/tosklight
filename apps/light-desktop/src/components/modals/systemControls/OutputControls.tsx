import { type KeyboardEvent, useState } from "react";
import { Button, SelectField } from "@tosklight/ui";
import { VerticalTouchFaderSurface } from "@tosklight/ui/faders";
import type { ControlActionSemantic } from "../../../api/types";

const PRIMARY_FIXTURE_ACTIONS = [
	{
		semantic: "lamp_on",
		all: "All Lamps On",
		selected: "Selected Lamps On",
		danger: false,
	},
	{
		semantic: "lamp_off",
		all: "All Lamps Off",
		selected: "Selected Lamps Off",
		danger: false,
	},
	{
		semantic: "reset",
		all: "Reset All Fixtures",
		selected: "Reset Selected Fixtures",
		danger: true,
	},
] as const;

const FAN_MODE_ACTIONS = [
	{
		semantic: "fan_auto",
		all: "All Fans Auto",
		selected: "Selected Fans Auto",
		danger: false,
	},
	{
		semantic: "fan_low",
		all: "All Fans Low",
		selected: "Selected Fans Low",
		danger: false,
	},
	{
		semantic: "fan_high",
		all: "All Fans High",
		selected: "Selected Fans High",
		danger: false,
	},
	{
		semantic: "fan_max",
		all: "All Fans Max",
		selected: "Selected Fans Max",
		danger: false,
	},
] as const;

export const FIXED_FIXTURE_ACTIONS = [
	...PRIMARY_FIXTURE_ACTIONS,
	...FAN_MODE_ACTIONS,
] as const satisfies readonly {
	semantic: ControlActionSemantic;
	all: string;
	selected: string;
	danger?: boolean;
}[];

interface OutputControlsProps {
	master: number | null;
	blackout: boolean;
	ready: boolean;
	fixtureActionResult: string;
	fixturesSelected: boolean;
	availableFixtureActions: ReadonlySet<ControlActionSemantic>;
	onMaster(value: number): void;
	onBlackout(): void;
	onFixtureAction(
		semantic: ControlActionSemantic,
		phase: "click" | "press" | "release",
	): void;
}

export function OutputControls(props: OutputControlsProps) {
	const [view, setView] = useState<"masters" | "actions">("masters");
	const [fanMode, setFanMode] = useState<
		"fan_mode" | "fan_auto" | "fan_low" | "fan_high" | "fan_max"
	>("fan_mode");
	return (
		<section className="system-controls-output">
			<div className="system-controls-left-rail">
				{view === "masters" ? (
					<MasterControls {...props} onActions={() => setView("actions")} />
				) : (
					<div
						className="fixture-control-actions"
						aria-label="Fixture controls"
					>
						<Button
							className="fixture-actions-back"
							onClick={() => setView("masters")}
						>
							Masters
						</Button>
						{PRIMARY_FIXTURE_ACTIONS.map((action) => (
							<FixtureActionButton
								key={action.semantic}
								label={
									props.fixturesSelected ? action.selected : action.all
								}
								danger={action.danger}
								disabled={
									!props.availableFixtureActions.has(action.semantic)
								}
								onAction={(phase) =>
									props.onFixtureAction(action.semantic, phase)
								}
							/>
						))}
						<span className="fixture-actions-spacer" />
						<SelectField
							label="Fan Mode"
							ariaLabel="Fan Mode"
							value={fanMode}
							options={[
								{ value: "fan_mode", label: "Fan Mode", disabled: true },
								...FAN_MODE_ACTIONS.map((action) => ({
									value: action.semantic,
									label: `Fan Mode ${action.semantic
										.slice("fan_".length)
										.replace(/^./, (letter) => letter.toUpperCase())}`,
									disabled: !props.availableFixtureActions.has(
										action.semantic,
									),
								})),
							]}
							onChange={(semantic) => {
								if (semantic === "fan_mode") return;
								setFanMode(semantic);
								props.onFixtureAction(semantic, "click");
							}}
						/>
					</div>
				)}
				{props.fixtureActionResult && (
					<p className="fixture-command-result" role="status">
						{props.fixtureActionResult}
					</p>
				)}
			</div>
		</section>
	);
}

function MasterControls(
	props: OutputControlsProps & { onActions(): void },
) {
	return (
		<div className="master-controls" aria-label="Master controls">
			<VerticalTouchFaderSurface
				label="Grand Master"
				value={props.master ?? 0}
				display={props.ready ? undefined : "—"}
				disabled={!props.ready}
				onChange={props.onMaster}
				hardware={false}
			/>
			<Button
				className={props.blackout ? "danger active" : "danger"}
				disabled={!props.ready}
				onClick={props.onBlackout}
			>
				{props.blackout ? "RELEASE BLACKOUT" : "BLACKOUT"}
			</Button>
			<Button className="fixture-actions-open" onClick={props.onActions}>
				Actions
			</Button>
		</div>
	);
}

function FixtureActionButton({
	label,
	danger = false,
	disabled,
	onAction,
}: {
	label: string;
	danger?: boolean;
	disabled: boolean;
	onAction(phase: "click" | "press" | "release"): void;
}) {
	const keyDown = (event: KeyboardEvent) => {
		if (!event.repeat && (event.key === "Enter" || event.key === " "))
			onAction("press");
	};
	const keyUp = (event: KeyboardEvent) => {
		if (event.key === "Enter" || event.key === " ") onAction("release");
	};
	return (
		<Button
			className={danger ? "danger" : undefined}
			disabled={disabled}
			onClick={() => onAction("click")}
			onPointerDown={() => onAction("press")}
			onPointerUp={() => onAction("release")}
			onPointerCancel={() => onAction("release")}
			onPointerLeave={() => onAction("release")}
			onKeyDown={keyDown}
			onKeyUp={keyUp}
		>
			{label}
		</Button>
	);
}
