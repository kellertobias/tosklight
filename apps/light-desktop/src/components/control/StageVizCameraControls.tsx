import { Button } from "@tosklight/ui";
import { useEffect, useState } from "react";
import { useDesktopBridge } from "../../platform/desktop";
import { HardwareEncoderDisplay } from "./HardwareEncoderDisplay";
import { VerticalTouchFader } from "./VerticalTouchFader";

/**
 * Aiming the renderer's camera from the encoders.
 *
 * The camera belongs to the renderer — it is the side that drags, orbits and clamps it — so this
 * reads where it is rather than keeping a copy. A desk tracking its own would drift the moment an
 * operator touched the pane with a mouse, and the encoders would then be showing numbers that are
 * not what anybody is looking at.
 *
 * Two pages, because six numbers do not belong together: where the camera is, and where it looks.
 */
const PAGES = [
	{
		label: "Position",
		// A fader runs from zero, so a value that can be negative is carried with an offset and
		// taken back off on the way out — the same way the desk's other stage controls do it.
		encoders: [
			{ key: "x", label: "X Pos", unit: "m", offset: 50, range: 100 },
			{ key: "y", label: "Y Pos", unit: "m", offset: 20, range: 60 },
			{ key: "z", label: "Z Pos", unit: "m", offset: 50, range: 100 },
			{ key: "distance", label: "Zoom", unit: "m", offset: 0, range: 80 },
		],
	},
	{
		label: "Direction",
		encoders: [
			{ key: "pan", label: "Pan", unit: "°", offset: 180, range: 360 },
			{ key: "tilt", label: "Tilt", unit: "°", offset: 89, range: 178 },
		],
	},
] as const;

/** How often the camera is read back. Fast enough to follow a drag without flooding the channel. */
const READ_INTERVAL = 120;

type Camera = Record<string, number>;

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

	// Only the number that moved is sent. The others are left alone rather than re-asserted, so a
	// mouse drag happening at the same time is not fought by an encoder that was not touched.
	const place = (key: string, value: number) => {
		setCamera((current) => (current ? { ...current, [key]: value } : current));
		void bridge.placeStagePaneCamera({ [key]: value });
	};

	const current = PAGES[page];
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
				{current.encoders.map((encoder, slot) =>
					hardwareConnected ? (
						<HardwareEncoderDisplay
							key={encoder.key}
							slot={slot + 1}
							target={{
								label: encoder.label,
								value:
									camera == null
										? "—"
										: `${camera[encoder.key].toFixed(1)}${encoder.unit}`,
								role: "Turn",
							}}
						/>
					) : (
						<VerticalTouchFader
							key={encoder.key}
							label={encoder.label}
							value={(camera?.[encoder.key] ?? 0) + encoder.offset}
							maximum={encoder.range}
							display={
								camera == null
									? "—"
									: `${camera[encoder.key].toFixed(1)}${encoder.unit}`
							}
							onChange={(value) => place(encoder.key, value - encoder.offset)}
						/>
					),
				)}
				{/* The rest of the bank is empty rather than repeating: a camera has six numbers. */}
				{Array.from(
					{ length: 6 - current.encoders.length },
					(_, index) =>
						hardwareConnected && (
							<HardwareEncoderDisplay
								key={`empty-${index}`}
								slot={current.encoders.length + index + 1}
							/>
						),
				)}
			</div>
		</div>
	);
}
