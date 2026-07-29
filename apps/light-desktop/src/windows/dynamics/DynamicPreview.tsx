import { Button, MultiValueToggle } from "@tosklight/ui";
import { type CSSProperties, useMemo } from "react";
import type {
	DynamicDefinitionProjection,
	DynamicLaneProjection,
	DynamicPeriodicFunctionProjection,
	DynamicPhaseOrderingProjection,
	DynamicScalarSourceProjection,
	SpeedGroupId,
} from "../../api/types";
import type { ShowObject } from "../../features/showObjects/contracts";

type DynamicObject = ShowObject<"dynamic">;

export function DynamicSelectionPreview({
	dynamic,
	previewPhase,
	selection,
	positions,
	positions3d,
}: {
	dynamic: DynamicObject;
	previewPhase: number;
	selection: readonly string[];
	positions: Record<string, { x: number; y: number; rotation: number }>;
	positions3d: Record<string, { x: number; z: number }>;
}) {
	const selected = selection.length > 0;
	const previewPositions = useMemo(
		() =>
			selected
				? normalizeSelectedPreviewPositions(selection, positions, positions3d)
				: virtualFixturePreviewPositions(),
		[selected, selection, positions, positions3d],
	);
	const phaseOffsets = useMemo(
		() => dynamicPreviewPhaseOffsets(dynamic.body.phase, previewPositions),
		[dynamic.body.phase, previewPositions],
	);
	return (
		<aside
			className="dynamic-face-preview-sidebar dynamic-discussion-preview-sidebar"
			aria-label="Selection preview"
		>
			<header>
				<span>
					<strong>{selected ? "Selected fixtures" : "Virtual fixtures"}</strong>
					<small>
						{selected
							? "Top-down Stage projection"
							: "Front-end-only 20 × 20 grid"}
					</small>
				</span>
				<b>{selected ? selection.length : 400}</b>
			</header>
			<div
				className={`dynamic-face-fixture-field ${selected ? "selected-fixtures" : "virtual-fixtures"}`}
				role="img"
				aria-label={
					selected
						? `Top-down preview of ${selection.length} selected fixtures`
						: "Front-end-only preview of 400 virtual fixtures"
				}
			>
				{previewPositions.map(({ id, left, top }, index) => {
					const values = dynamicPreviewValues(
						dynamic.body,
						moduloOne(previewPhase + (phaseOffsets[index] ?? 0)),
					);
					return (
						<i
							key={id}
							className="dynamic-face-fixture"
							style={
								{
									left: `${left}%`,
									top: `${top}%`,
									"--fixture-color": `rgb(${Math.round(values.red * 255)} ${Math.round(values.green * 255)} ${Math.round(values.blue * 255)})`,
									"--fixture-intensity": values.intensity,
									"--pan": `${(values.pan - 0.5) * 56}%`,
									"--tilt": `${(values.tilt - 0.5) * 56}%`,
								} as CSSProperties
							}
						>
							<span />
						</i>
					);
				})}
			</div>
		</aside>
	);
}

function normalizeSelectedPreviewPositions(
	selection: readonly string[],
	positions: Record<string, { x: number; y: number }>,
	positions3d: Record<string, { x: number; z: number }>,
) {
	const fallback = gridFixturePreviewPositions(selection.length);
	const resolved = selection.map((id, index) => {
		const position3d = positions3d[id];
		if (position3d)
			return { id, x: position3d.x, z: position3d.z, fallback: false };
		const position2d = positions[id];
		if (position2d)
			return { id, x: position2d.x, z: position2d.y, fallback: false };
		return {
			id,
			x: fallback[index]?.left ?? 50,
			z: fallback[index]?.top ?? 50,
			fallback: true,
		};
	});
	const positioned = resolved.filter((position) => !position.fallback);
	if (positioned.length === 0) return fallback;
	const xs = positioned.map(({ x }) => x);
	const zs = positioned.map(({ z }) => z);
	const minX = Math.min(...xs);
	const maxX = Math.max(...xs);
	const minZ = Math.min(...zs);
	const maxZ = Math.max(...zs);
	return resolved.map((position, index) =>
		position.fallback
			? (fallback[index] ?? { id: position.id, left: 50, top: 50 })
			: {
					id: position.id,
					left: normalizePreviewCoordinate(position.x, minX, maxX),
					top: normalizePreviewCoordinate(position.z, minZ, maxZ),
				},
	);
}

function normalizePreviewCoordinate(
	value: number,
	minimum: number,
	maximum: number,
) {
	if (maximum === minimum) return 50;
	return 8 + ((value - minimum) / (maximum - minimum)) * 84;
}

function gridFixturePreviewPositions(count: number) {
	const columns = Math.max(1, Math.ceil(Math.sqrt(count)));
	const rows = Math.max(1, Math.ceil(count / columns));
	return Array.from({ length: count }, (_, index) => ({
		id: `fixture-${index}`,
		left: columns === 1 ? 50 : 8 + (index % columns) * (84 / (columns - 1)),
		top: rows === 1 ? 50 : 8 + Math.floor(index / columns) * (84 / (rows - 1)),
	}));
}

function virtualFixturePreviewPositions() {
	return Array.from({ length: 400 }, (_, index) => ({
		id: `virtual-fixture-${index}`,
		left: 5 + (index % 20) * (90 / 19),
		top: 5 + Math.floor(index / 20) * (90 / 19),
	}));
}

function dynamicPreviewPhaseOffsets(
	phase: DynamicDefinitionProjection["phase"],
	positions: readonly { id: string; left: number; top: number }[],
) {
	const ordered = positions.map((position, index) => ({ position, index }));
	const center = { left: 50, top: 50 };
	switch (phase.ordering.type) {
		case "grid_linear": {
			const radians = (phase.ordering.angle_degrees * Math.PI) / 180;
			ordered.sort(
				(left, right) =>
					left.position.left * Math.cos(radians) +
					left.position.top * Math.sin(radians) -
					(right.position.left * Math.cos(radians) +
						right.position.top * Math.sin(radians)),
			);
			break;
		}
		case "radial_out":
		case "radial_in":
			ordered.sort((left, right) => {
				const distance = (item: (typeof ordered)[number]) =>
					Math.hypot(
						item.position.left - center.left,
						item.position.top - center.top,
					);
				const comparison = distance(left) - distance(right);
				return phase.ordering.type === "radial_in" ? -comparison : comparison;
			});
			break;
		case "axial":
			ordered.sort(
				(left, right) =>
					Math.atan2(
						left.position.top - center.top,
						left.position.left - center.left,
					) -
					Math.atan2(
						right.position.top - center.top,
						right.position.left - center.left,
					),
			);
			break;
		case "random_each_loop": {
			const seed = phase.ordering.seed;
			ordered.sort(
				(left, right) =>
					previewHash(left.position.id, seed) -
					previewHash(right.position.id, seed),
			);
			break;
		}
		case "selection":
			break;
	}
	const blockSize = Math.max(1, Math.round(phase.block_size));
	const rankCount = Math.ceil(ordered.length / blockSize);
	const repeats = Math.min(Math.max(1, Math.round(phase.repeats)), rankCount);
	const offsets = Array.from({ length: positions.length }, () => 0);
	for (const [spatialRank, entry] of ordered.entries()) {
		const rank = Math.floor(spatialRank / blockSize);
		const { local, length } = balancedPreviewRepeat(rank, rankCount, repeats);
		const wingLocal = phase.wings
			? Math.min(local, Math.max(0, length - 1 - local))
			: local;
		const effectiveLength = phase.wings ? Math.ceil(length / 2) : length;
		const distributed =
			phase.anchors_degrees.length >= 2
				? previewAnchorPhase(phase.anchors_degrees, wingLocal, effectiveLength)
				: effectiveLength <= 1
					? 0
					: (wingLocal / effectiveLength) * phase.span_degrees;
		offsets[entry.index] = (phase.offset_degrees + distributed) / 360;
	}
	return offsets;
}

function balancedPreviewRepeat(rank: number, count: number, repeats: number) {
	const base = Math.floor(count / repeats);
	const extras = count % repeats;
	let start = 0;
	for (let repeat = 0; repeat < repeats; repeat += 1) {
		const length = base + (repeat < extras ? 1 : 0);
		if (rank < start + length) return { local: rank - start, length };
		start += length;
	}
	return { local: 0, length: 1 };
}

function previewAnchorPhase(
	anchors: readonly number[],
	local: number,
	length: number,
) {
	if (length <= 1) return anchors[0] ?? 0;
	const progress = local / length;
	const position = progress * (anchors.length - 1);
	const leftIndex = Math.min(Math.floor(position), anchors.length - 2);
	const mix = position - leftIndex;
	return (
		(anchors[leftIndex] ?? 0) * (1 - mix) + (anchors[leftIndex + 1] ?? 0) * mix
	);
}

function previewHash(value: string, seed: number) {
	let hash = seed | 0;
	for (const character of value)
		hash = Math.imul(hash ^ character.charCodeAt(0), 16_777_619);
	return hash >>> 0;
}

function dynamicPreviewValues(
	dynamic: DynamicDefinitionProjection,
	phase: number,
) {
	const values = new Map<string, number>();
	for (const lane of dynamic.lanes)
		values.set(lane.attribute, lanePreviewValue(lane, dynamic, phase));
	const hasColor = [...values.keys()].some((attribute) =>
		attribute.startsWith("color."),
	);
	return {
		intensity: clamp(values.get("intensity") ?? 1, 0, 1),
		red: clamp(values.get("color.red") ?? (hasColor ? 0 : 0.2), 0, 1),
		green: clamp(values.get("color.green") ?? (hasColor ? 0 : 0.78), 0, 1),
		blue: clamp(values.get("color.blue") ?? 1, 0, 1),
		pan: clamp(values.get("pan") ?? 0.5, 0, 1),
		tilt: clamp(values.get("tilt") ?? 0.5, 0, 1),
	};
}

function lanePreviewValue(
	lane: DynamicLaneProjection,
	dynamic: DynamicDefinitionProjection,
	phase: number,
) {
	const intervalPhase = moduloOne(phase * rationalValue(lane.speed_multiplier));
	const functionName =
		lane.mode === "middle_amplitude"
			? lane.middle_amplitude.function
			: lane.mode === "max_min"
				? lane.max_min.function
				: null;
	const width = functionName === "pwm" ? 1 : clamp(lane.width, 0.05, 1);
	const position = clamp((intervalPhase - (1 - width) / 2) / width, 0, 1);
	if (lane.mode === "keyframes")
		return keyframePreviewValue(lane.keyframes.points, position);
	if (lane.mode === "random") {
		const group = dynamic.random_groups.find(
			(candidate) => candidate.id === lane.random_group_id,
		);
		if (!group) return 0;
		const low = scalarSourceCurveValue(group.low);
		const high = scalarSourceCurveValue(group.high);
		return previewHash(lane.id, Math.floor(position * 1_000_000)) % 2
			? high
			: low;
	}
	const shape = periodicPreviewValue(
		functionName ?? "sinus",
		position,
		lane.mode === "middle_amplitude"
			? lane.middle_amplitude.pwm
			: lane.max_min.pwm,
	);
	if (lane.mode === "middle_amplitude") {
		const middle = scalarSourceCurveValue(lane.middle_amplitude.middle);
		return clamp(
			middle +
				(shape * 2 - 1) *
					lane.middle_amplitude.amplitude *
					lane.middle_amplitude.size,
			0,
			1,
		);
	}
	const minimum = scalarSourceCurveValue(lane.max_min.minimum);
	const maximum = scalarSourceCurveValue(lane.max_min.maximum);
	const middle = (minimum + maximum) / 2;
	return clamp(
		middle +
			(minimum + (maximum - minimum) * shape - middle) * lane.max_min.size,
		0,
		1,
	);
}

function keyframePreviewValue(
	points: readonly {
		position: number;
		source: DynamicScalarSourceProjection;
		interpolation: string;
	}[],
	position: number,
) {
	if (points.length === 0) return 0;
	let leftIndex = 0;
	for (let index = 0; index < points.length; index += 1) {
		if ((points[index]?.position ?? 1) > position) break;
		leftIndex = index;
	}
	const left = points[leftIndex] ?? points[0];
	const right = points[leftIndex + 1] ?? points[0];
	if (!left || !right) return 0;
	const rightPosition = leftIndex + 1 < points.length ? right.position : 1;
	const progress = clamp(
		(position - left.position) /
			Math.max(Number.EPSILON, rightPosition - left.position),
		0,
		1,
	);
	const mix = interpolationPreviewValue(progress, left.interpolation);
	return clamp(
		scalarSourceCurveValue(left.source) * (1 - mix) +
			scalarSourceCurveValue(right.source) * mix,
		0,
		1,
	);
}

export function periodicPreviewValue(
	functionName: DynamicPeriodicFunctionProjection,
	position: number,
	pwm: DynamicLaneProjection["max_min"]["pwm"],
) {
	switch (functionName) {
		case "linear_up":
			return position;
		case "linear_down":
			return 1 - position;
		case "cosinus":
			return (Math.cos(position * Math.PI * 2) + 1) / 2;
		case "pwm":
			return pwmPreviewValue(position, pwm);
		case "sinus":
			return (Math.sin(position * Math.PI * 2 - Math.PI / 2) + 1) / 2;
	}
}

function pwmPreviewValue(
	position: number,
	pwm: DynamicLaneProjection["max_min"]["pwm"],
) {
	const total = Math.max(Number.EPSILON, pwm.on + pwm.off);
	const onEnd = pwm.on / total;
	const attackEnd = Math.min(pwm.attack / total, onEnd);
	const decayEnd = Math.min(1, onEnd + pwm.decay / total);
	if (attackEnd > 0 && position < attackEnd)
		return interpolationPreviewValue(
			position / attackEnd,
			pwm.attack_interpolation,
		);
	if (position < onEnd) return 1;
	if (decayEnd > onEnd && position < decayEnd)
		return (
			1 -
			interpolationPreviewValue(
				(position - onEnd) / (decayEnd - onEnd),
				pwm.decay_interpolation,
			)
		);
	return 0;
}

function interpolationPreviewValue(progress: number, interpolation: string) {
	switch (interpolation) {
		case "ease_in":
			return progress * progress;
		case "ease_out":
			return 1 - (1 - progress) * (1 - progress);
		case "ease_in_out":
			return progress * progress * (3 - 2 * progress);
		case "hold":
			return 0;
		case "drop":
			return progress >= 1 ? 1 : 0;
		default:
			return progress;
	}
}

function moduloOne(value: number) {
	return ((value % 1) + 1) % 1;
}

export function dynamicPreviewCycleMillis(
	dynamic: DynamicDefinitionProjection,
	speedGroupBpms?: Partial<Record<SpeedGroupId, number>>,
) {
	const baseCycleMillis =
		dynamic.speed.type === "fixed"
			? dynamic.speed.duration_millis
			: (60_000 / Math.max(1, speedGroupBpms?.[dynamic.speed.group] ?? 120)) *
				rationalValue(dynamic.speed.beats_per_cycle);
	return Math.max(
		1,
		baseCycleMillis /
			Math.max(Number.EPSILON, rationalValue(dynamic.overall_speed_multiplier)),
	);
}

export function DynamicPhaseQuickControls({
	phase,
	running,
	selectionCount,
	targetless,
	onPhasePatch,
	onTakeSelection,
	onClearSelection,
}: {
	phase: DynamicObject["body"]["phase"];
	running: boolean;
	selectionCount: number;
	targetless: boolean;
	onPhasePatch(patch: Partial<DynamicObject["body"]["phase"]>): void;
	onTakeSelection(): void;
	onClearSelection(): void;
}) {
	const orderingValue =
		phase.ordering.type === "radial_in" ? "radial_out" : phase.ordering.type;
	return (
		<fieldset
			className="dynamic-discussion-phase-controls"
			aria-label="Phase quick controls"
		>
			<div className="dynamic-discussion-phase-ordering">
				<MultiValueToggle
					ariaLabel="Ordering mode"
					value={orderingValue}
					options={[
						{ value: "selection", label: "Linear" },
						{ value: "grid_linear", label: "Grid" },
						{ value: "radial_out", label: "Radial" },
						{ value: "axial", label: "Radar" },
						{ value: "random_each_loop", label: "Random" },
					]}
					onChange={(type) => {
						const center =
							phase.ordering.type === "radial_out" ||
							phase.ordering.type === "radial_in" ||
							phase.ordering.type === "axial"
								? {
										center_x: phase.ordering.center_x,
										center_z: phase.ordering.center_z,
									}
								: { center_x: 0, center_z: 0 };
						onPhasePatch({
							ordering:
								type === "grid_linear"
									? { type, angle_degrees: 90 }
									: type === "radial_out" || type === "axial"
										? { type, ...center }
										: type === "random_each_loop"
											? { type, seed: 201 }
											: { type: "selection" },
						});
					}}
				/>
			</div>
			<div className="dynamic-discussion-phase-actions">
				<Button
					disabled={running || selectionCount === 0}
					onClick={onTakeSelection}
				>
					Take Selection
				</Button>
				<Button disabled={running || targetless} onClick={onClearSelection}>
					Clear Selection
				</Button>
			</div>
		</fieldset>
	);
}

function clamp(value: number, minimum: number, maximum: number) {
	return Math.max(minimum, Math.min(maximum, value));
}

function rationalValue(value: { numerator: number; denominator: number }) {
	return value.numerator / value.denominator;
}

function scalarSourceCurveValue(
	source: DynamicScalarSourceProjection | undefined,
) {
	return source?.type === "value" ? source.value : 0.5;
}
