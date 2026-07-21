import { type PointerEvent, useRef, useState } from "react";
import { useSelectedPatchedFixtures } from "../../../features/patch/PatchState";
import { useServer } from "../../../api/ServerContext";
import {
	normalizedFixtureMutations,
	programmerValuesMutationKey,
	type ProgrammerValuesMutationQueueController,
} from "../../../features/programmerValues/useProgrammerValuesMutationQueue";
import { Button } from "../../common";
import {
	colorProgrammerAssignments,
	hsvToRgb,
	interpolatePickerRange,
	type PickerColor,
} from "../specialColor";
import { normalizedPointerPosition } from "./pointer";

interface ColorRangePreview {
	start: PickerColor;
	end: PickerColor;
	active: boolean;
}

interface ColorDialogController {
	brightness: number;
	colorRangePreview: ColorRangePreview | null;
	colorSheet: React.RefObject<HTMLDivElement | null>;
	hue: number;
	saturation: number;
	swatch: string;
	disabled: boolean;
	cancelColor: (event: PointerEvent<HTMLDivElement>) => void;
	changeBrightness: (delta: number) => void;
	completeColor: (event: PointerEvent<HTMLDivElement>) => void;
	moveColor: (event: PointerEvent<HTMLDivElement>) => void;
	startColor: (event: PointerEvent<HTMLDivElement>) => void;
}

export function useColorDialog(
	selectedFixtureIds: readonly string[],
	shiftArmed: boolean,
	valueWrites: ProgrammerValuesMutationQueueController,
): ColorDialogController {
	const server = useServer();
	const [hue, setHue] = useState(0.52);
	const [saturation, setSaturation] = useState(0.8);
	const [brightness, setBrightness] = useState(0.85);
	const [colorRangePreview, setColorRangePreview] =
		useState<ColorRangePreview | null>(null);
	const colorSheet = useRef<HTMLDivElement>(null);
	const colorRangeGesture = useRef<{
		pointerId: number;
		start: PickerColor;
	} | null>(null);
	const selectedFixtures = useSelectedPatchedFixtures(selectedFixtureIds);

	const applyColors = async (
		colors: PickerColor[],
		mode: "latest" | "barrier",
	) => {
		const assignments = colorProgrammerAssignments(
			selectedFixtureIds,
			selectedFixtures,
			colors,
		);
		const mutations = normalizedFixtureMutations(
			assignments,
			server.configuration?.programmer_fade_millis,
		);
		if (!mutations.length) return;
		if (mode === "barrier") await valueWrites.submitBarrier(mutations);
		else
			await valueWrites.submitLatest(
				programmerValuesMutationKey(mutations),
				mutations,
			);
	};

	const pickerColor = (event: PointerEvent<HTMLDivElement>): PickerColor => {
		const next = normalizedPointerPosition(event, colorSheet);
		return { hue: next.x, saturation: 1 - next.y, brightness };
	};

	const moveColor = (event: PointerEvent<HTMLDivElement>) => {
		if (!valueWrites.canWrite) return;
		const next = pickerColor(event);
		setHue(next.hue);
		setSaturation(next.saturation);
		const gesture = colorRangeGesture.current;
		if (gesture?.pointerId === event.pointerId) {
			setColorRangePreview({ start: gesture.start, end: next, active: true });
			return;
		}
		void applyColors(
			selectedFixtureIds.map(() => next),
			"latest",
		);
	};

	const startColor = (event: PointerEvent<HTMLDivElement>) => {
		if (!valueWrites.canWrite) return;
		event.currentTarget.setPointerCapture(event.pointerId);
		const start = pickerColor(event);
		if (event.shiftKey || shiftArmed) {
			colorRangeGesture.current = { pointerId: event.pointerId, start };
			setHue(start.hue);
			setSaturation(start.saturation);
			setColorRangePreview({ start, end: start, active: true });
			return;
		}
		moveColor(event);
	};

	const completeColor = (event: PointerEvent<HTMLDivElement>) => {
		const gesture = colorRangeGesture.current;
		if (!gesture || gesture.pointerId !== event.pointerId) return;
		const end = pickerColor(event);
		colorRangeGesture.current = null;
		setHue(end.hue);
		setSaturation(end.saturation);
		setColorRangePreview({ start: gesture.start, end, active: false });
		if (!valueWrites.canWrite) return;
		void applyColors(
			interpolatePickerRange(selectedFixtureIds.length, gesture.start, end),
			"barrier",
		);
	};

	const cancelColor = (event: PointerEvent<HTMLDivElement>) => {
		if (colorRangeGesture.current?.pointerId !== event.pointerId) return;
		colorRangeGesture.current = null;
		setColorRangePreview(null);
	};

	const changeBrightness = (delta: number) => {
		if (!valueWrites.canWrite) return;
		const value = Math.max(0, Math.min(1, brightness + delta));
		setBrightness(value);
		void applyColors(
			selectedFixtureIds.map(() => ({
				hue,
				saturation,
				brightness: value,
			})),
			"barrier",
		);
	};

	const color = hsvToRgb({ hue, saturation, brightness });
	const swatch = `rgb(${color.map((channel) => Math.round(channel * 255)).join(",")})`;
	return {
		brightness,
		colorRangePreview,
		colorSheet,
		hue,
		saturation,
		swatch,
		disabled: !valueWrites.canWrite,
		cancelColor,
		changeBrightness,
		completeColor,
		moveColor,
		startColor,
	};
}

interface ColorDialogProps extends ColorDialogController {
	shiftArmed: boolean;
}

export function ColorDialog({
	brightness,
	colorRangePreview,
	colorSheet,
	hue,
	saturation,
	swatch,
	disabled,
	shiftArmed,
	cancelColor,
	changeBrightness,
	completeColor,
	moveColor,
	startColor,
}: ColorDialogProps) {
	return (
		<div className="graphical-color-picker">
			<div
				ref={colorSheet}
				className="color-sheet"
				data-range-shift={shiftArmed ? "armed" : "idle"}
				aria-disabled={disabled}
				style={{ backgroundColor: `hsl(${hue * 360} 100% 50%)` }}
				onPointerDown={startColor}
				onPointerMove={(event) => {
					if (event.currentTarget.hasPointerCapture(event.pointerId))
						moveColor(event);
				}}
				onPointerUp={completeColor}
				onPointerCancel={cancelColor}
				onLostPointerCapture={cancelColor}
			>
				{colorRangePreview && (
					<ColorRangeIndicator preview={colorRangePreview} />
				)}
				<i
					style={{
						left: `${hue * 100}%`,
						top: `${(1 - saturation) * 100}%`,
					}}
				/>
			</div>
			<div className="brightness-control">
				<span>Brightness</span>
				<Button
					aria-label="Decrease brightness"
					disabled={disabled}
					onClick={() => changeBrightness(-0.05)}
				>
					−
				</Button>
				<b>{Math.round(brightness * 100)}%</b>
				<Button
					aria-label="Increase brightness"
					disabled={disabled}
					onClick={() => changeBrightness(0.05)}
				>
					+
				</Button>
			</div>
			<strong style={{ color: swatch }}>{swatch}</strong>
		</div>
	);
}

function ColorRangeIndicator({ preview }: { preview: ColorRangePreview }) {
	return (
		<svg
			className="color-range-preview"
			data-active={preview.active}
			viewBox="0 0 100 100"
			preserveAspectRatio="none"
			style={{
				position: "absolute",
				zIndex: 2,
				inset: 0,
				width: "100%",
				height: "100%",
				overflow: "visible",
				pointerEvents: "none",
			}}
			aria-hidden="true"
		>
			<line
				x1={preview.start.hue * 100}
				y1={(1 - preview.start.saturation) * 100}
				x2={preview.end.hue * 100}
				y2={(1 - preview.end.saturation) * 100}
				stroke="white"
				strokeWidth="1.5"
				strokeDasharray={preview.active ? "3 2" : undefined}
				vectorEffect="non-scaling-stroke"
			/>
			<circle
				cx={preview.start.hue * 100}
				cy={(1 - preview.start.saturation) * 100}
				r="2.5"
				fill="white"
				vectorEffect="non-scaling-stroke"
			/>
			<circle
				cx={preview.end.hue * 100}
				cy={(1 - preview.end.saturation) * 100}
				r="2.5"
				fill="none"
				stroke="white"
				strokeWidth="1.5"
				vectorEffect="non-scaling-stroke"
			/>
		</svg>
	);
}
