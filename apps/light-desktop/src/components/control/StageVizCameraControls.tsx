import { Button } from "@tosklight/ui";
import { TouchEncoder } from "@tosklight/ui/encoders";
import { useCallback, useEffect, useState } from "react";
import { useDesktopBridge } from "../../platform/desktop";
import { HardwareEncoderDisplay } from "./HardwareEncoderDisplay";

const PAGES = [
	{
		label: "Position",
		encoders: [
			{
				key: "x",
				label: "X Pos",
				unit: "m",
				minimum: -50,
				maximum: 50,
				fineStep: 0.1,
				coarseStep: 1,
			},
			{
				key: "y",
				label: "Y Pos",
				unit: "m",
				minimum: -20,
				maximum: 40,
				fineStep: 0.1,
				coarseStep: 1,
			},
			{
				key: "z",
				label: "Z Pos",
				unit: "m",
				minimum: -50,
				maximum: 50,
				fineStep: 0.1,
				coarseStep: 1,
			},
			{
				key: "distance",
				label: "Zoom",
				unit: "m",
				minimum: 0,
				maximum: 80,
				fineStep: 0.1,
				coarseStep: 1,
			},
		],
	},
	{
		label: "Direction",
		encoders: [
			{
				key: "pan",
				label: "Pan",
				unit: "°",
				minimum: -180,
				maximum: 180,
				fineStep: 1,
				coarseStep: 10,
			},
			{
				key: "tilt",
				label: "Tilt",
				unit: "°",
				minimum: -89,
				maximum: 89,
				fineStep: 1,
				coarseStep: 10,
			},
		],
	},
] as const;

const READ_INTERVAL = 120;

type CameraKey = (typeof PAGES)[number]["encoders"][number]["key"];
type Camera = Record<CameraKey, number>;

export function StageVizCameraControls({
	hardwareConnected,
}: {
	hardwareConnected: boolean;
}) {
	const bridge = useDesktopBridge();
	const [page, setPage] = useState(0);
	const [camera, setCamera] = useState<Camera | null>(null);

	useEffect(() => {
		let cancelled = false;
		const read = async () => {
			const reported = await bridge.stagePaneCamera();
			if (cancelled || !reported) return;
			const [x, y, z, pan, tilt, distance] = reported;
			setCamera({ x, y, z, pan, tilt, distance });
		};
		void read();
		const timer = window.setInterval(() => void read(), READ_INTERVAL);
		return () => {
			cancelled = true;
			window.clearInterval(timer);
		};
	}, [bridge]);

	// Send only the number that moved, leaving simultaneous pointer camera changes alone.
	const place = useCallback(
		(key: CameraKey, value: number) => {
			setCamera((current) =>
				current ? { ...current, [key]: value } : current,
			);
			void bridge.placeStagePaneCamera({ [key]: value });
		},
		[bridge],
	);

	const current = PAGES[page];
	useEffect(() => {
		if (!hardwareConnected || !camera) return;
		const handleEncoder = (event: Event) => {
			const { control, value } = (
				event as CustomEvent<{ control: string; value?: string }>
			).detail;
			const encoder = current.encoders[Number(control.split("/")[1]) - 1];
			if (!encoder || !["up", "down", "left", "right"].includes(value ?? ""))
				return;
			const direction = value === "up" || value === "right" ? 1 : -1;
			const step =
				value === "left" || value === "right"
					? encoder.coarseStep
					: encoder.fineStep;
			place(
				encoder.key,
				Math.max(
					encoder.minimum,
					Math.min(encoder.maximum, camera[encoder.key] + direction * step),
				),
			);
		};
		window.addEventListener("light:encoder-action", handleEncoder);
		return () =>
			window.removeEventListener("light:encoder-action", handleEncoder);
	}, [camera, current, hardwareConnected, place]);

	return (
		<div className="parameter-controls stage-command-controls">
			<div className="family-tabs">
				{PAGES.map((entry, index) => (
					<Button
						key={entry.label}
						className={index === page ? "active" : ""}
						onClick={() => setPage(index)}
					>
						{entry.label}
					</Button>
				))}
			</div>
			<div className="parameter-surfaces">
				{current.encoders.map((encoder, index) => {
					const slot = index + 1;
					const value = camera?.[encoder.key] ?? 0;
					const display =
						camera == null ? "—" : `${value.toFixed(1)}${encoder.unit}`;
					const setValue = (next: number) =>
						place(
							encoder.key,
							Math.max(encoder.minimum, Math.min(encoder.maximum, next)),
						);
					return hardwareConnected ? (
						<HardwareEncoderDisplay
							key={encoder.key}
							slot={slot}
							activateOnHardwarePress
							target={{
								label: encoder.label,
								value: display,
								role: "Turn · Press-turn coarse",
							}}
							editValue={camera == null ? undefined : value}
							onEdit={camera == null ? undefined : (next) => setValue(next)}
						/>
					) : (
						<TouchEncoder
							key={encoder.key}
							label={`Enc ${slot} · ${encoder.label}`}
							slot={slot}
							attributeLabel={encoder.label}
							value={value}
							display={display}
							minimum={encoder.minimum}
							maximum={encoder.maximum}
							slowStep={encoder.fineStep}
							fastStep={encoder.coarseStep}
							disabled={camera == null}
							onStep={(delta) => setValue(value + delta)}
							onSet={setValue}
						/>
					);
				})}
				{hardwareConnected &&
					Array.from({ length: 6 - current.encoders.length }, (_, index) => (
						<HardwareEncoderDisplay
							key={`empty-${index}`}
							slot={current.encoders.length + index + 1}
						/>
					))}
			</div>
		</div>
	);
}
