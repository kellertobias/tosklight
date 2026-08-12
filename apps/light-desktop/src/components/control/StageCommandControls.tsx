import { Button } from "@tosklight/ui";
import { TouchEncoder } from "@tosklight/ui/encoders";
import { useEffect } from "react";
import { useHardwareConnected } from "../../features/deskSnapshot/DeskSnapshotState";
import { useApp } from "../../state/AppContext";
import { HardwareEncoderDisplay } from "./HardwareEncoderDisplay";

type NavigationKey =
	| "stageZoom"
	| "stagePanX"
	| "stagePanY"
	| "stageOrbitX"
	| "stageOrbitY";

interface NavigationEncoder {
	key: NavigationKey;
	label: string;
	minimum: number;
	maximum: number;
	fineStep: number;
	coarseStep: number;
	format(value: number): string;
}

const NAVIGATION_ENCODERS: readonly NavigationEncoder[] = [
	{
		key: "stageZoom",
		label: "Zoom",
		minimum: 0.2,
		maximum: 2,
		fineStep: 0.02,
		coarseStep: 0.2,
		format: (value) => `${Math.round(value * 100)}%`,
	},
	{
		key: "stagePanX",
		label: "X Pan",
		minimum: -100,
		maximum: 100,
		fineStep: 1,
		coarseStep: 5,
		format: (value) => String(Math.round(value)),
	},
	{
		key: "stagePanY",
		label: "Y Pan",
		minimum: -100,
		maximum: 100,
		fineStep: 1,
		coarseStep: 5,
		format: (value) => String(Math.round(value)),
	},
	{
		key: "stageOrbitX",
		label: "Orbit",
		minimum: -180,
		maximum: 180,
		fineStep: 1,
		coarseStep: 5,
		format: (value) => `${Math.round(value)}°`,
	},
	{
		key: "stageOrbitY",
		label: "Orbit tilt",
		minimum: -90,
		maximum: 90,
		fineStep: 1,
		coarseStep: 5,
		format: (value) => `${Math.round(value)}°`,
	},
];

const PLAN_NAVIGATION_ENCODERS = NAVIGATION_ENCODERS.slice(0, 3);

function clamp(value: number, encoder: NavigationEncoder) {
	return Math.max(encoder.minimum, Math.min(encoder.maximum, value));
}

function navigationAction(key: NavigationKey, value: number) {
	const property = {
		stageZoom: "zoom",
		stagePanX: "panX",
		stagePanY: "panY",
		stageOrbitX: "orbitX",
		stageOrbitY: "orbitY",
	} as const;
	return { type: "SET_STAGE_NAVIGATION" as const, [property[key]]: value };
}

export function StageCommandControls() {
	const { state, dispatch } = useApp();
	const hardwareAttached = useHardwareConnected();
	const hardwareConnected = Boolean(hardwareAttached || state.midiProfile);
	const encoders =
		state.stageView === "3d" ? NAVIGATION_ENCODERS : PLAN_NAVIGATION_ENCODERS;
	const setValue = (encoder: NavigationEncoder, value: number) =>
		dispatch(navigationAction(encoder.key, clamp(value, encoder)));

	useEffect(() => {
		if (!hardwareConnected || state.stageMode !== "navigate") return;
		const handleEncoder = (event: Event) => {
			const { control, value } = (
				event as CustomEvent<{ control: string; value?: string }>
			).detail;
			const encoder = encoders[Number(control.split("/")[1]) - 1];
			if (!encoder) return;
			const direction = value === "up" || value === "right" ? 1 : -1;
			if (!["up", "down", "left", "right"].includes(value ?? "")) return;
			const step =
				value === "left" || value === "right"
					? encoder.coarseStep
					: encoder.fineStep;
			setValue(encoder, state[encoder.key] + direction * step);
		};
		window.addEventListener("light:encoder-action", handleEncoder);
		return () =>
			window.removeEventListener("light:encoder-action", handleEncoder);
	}, [encoders, hardwareConnected, setValue, state]);

	if (state.stageMode !== "navigate") return null;
	return (
		<div className="parameter-controls stage-command-controls">
			<div className="family-tabs">
				<Button className="active">Navigate Stage</Button>
			</div>
			<div className="parameter-surfaces">
				{encoders.map((encoder, index) => {
					const slot = index + 1;
					const value = state[encoder.key];
					return hardwareConnected ? (
						<HardwareEncoderDisplay
							key={encoder.key}
							slot={slot}
							activateOnHardwarePress
							target={{
								label: encoder.label,
								value: encoder.format(value),
								role: "Turn · Press-turn coarse",
							}}
							editValue={value}
							onEdit={(next) => setValue(encoder, next)}
						/>
					) : (
						<TouchEncoder
							key={encoder.key}
							label={`Enc ${slot} · ${encoder.label}`}
							slot={slot}
							attributeLabel={encoder.label}
							value={value}
							display={encoder.format(value)}
							minimum={encoder.minimum}
							maximum={encoder.maximum}
							slowStep={encoder.fineStep}
							fastStep={encoder.coarseStep}
							onStep={(delta) => setValue(encoder, value + delta)}
							onSet={(next) => setValue(encoder, next)}
						/>
					);
				})}
				{hardwareConnected &&
					Array.from({ length: 6 - encoders.length }, (_, index) => (
						<HardwareEncoderDisplay
							key={`empty-${index}`}
							slot={encoders.length + index + 1}
						/>
					))}
			</div>
		</div>
	);
}
