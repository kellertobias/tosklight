import type { GeometryNode } from "../wire";
import {
	Button,
	CheckboxField,
	NumberField,
	SelectField,
	TextField,
} from "@tosklight/ui";
import { useState } from "react";
import { VectorFields } from "./geometryPreview";

type Motion = NonNullable<GeometryNode["motion"]>;

const TABS = [
	["generic", "Generic"],
	["translation", "Translation"],
	["rotation", "Rotation"],
	["scale", "Scale"],
	["pivot", "Pivot"],
	["animate", "Animate"],
] as const;
type PartTab = (typeof TABS)[number][0];

/**
 * How far and how fast the part can move.
 *
 * A yoke does not arrive instantly, and the speed figures are the difference between a move that
 * reads as the real fixture and one that snaps. Blank means the axis moves as fast as it is told to,
 * which is how every fixture behaved before these figures existed. Which attribute drives the move
 * belongs to a mode, because one mode's pan may be another mode's tilt.
 */
function GeometryMotionFields({
	motion,
	onChange,
}: {
	motion: Motion;
	onChange: (motion: Motion) => void;
}) {
	const unit = motion.kind === "rotation" ? "°" : "mm";
	return (
		<div className="geometry-motion">
			<SelectField
				label="Motion kind"
				value={motion.kind}
				options={[
					{ value: "rotation", label: "Rotation" },
					{ value: "translation", label: "Translation" },
				]}
				onChange={(kind) => onChange({ ...motion, kind })}
			/>
			<VectorFields
				label="Motion axis"
				value={motion.axis}
				onChange={(axis) => onChange({ ...motion, axis })}
			/>
			<NumberField
				label={`Physical minimum (${unit})`}
				allowDecimal
				value={motion.physical_min}
				onChange={(event) =>
					onChange({ ...motion, physical_min: Number(event.target.value) })
				}
			/>
			<NumberField
				label={`Physical maximum (${unit})`}
				allowDecimal
				value={motion.physical_max}
				onChange={(event) =>
					onChange({ ...motion, physical_max: Number(event.target.value) })
				}
			/>
			{(
				[
					["max_speed_per_second", "Top speed", `${unit}/s`],
					["acceleration_per_second_squared", "Acceleration", `${unit}/s²`],
					["deceleration_per_second_squared", "Deceleration", `${unit}/s²`],
				] as const
			).map(([key, label, suffix]) => (
				<NumberField
					key={key}
					label={`${label} (${suffix})`}
					allowDecimal
					min={0}
					value={motion[key] ?? ""}
					onChange={(event) => {
						const raw = event.target.value.trim();
						onChange({ ...motion, [key]: raw === "" ? null : Number(raw) });
					}}
				/>
			))}
		</div>
	);
}

/** One part's properties, a tab per concern so no tab needs scrolling to reach its end. */
export function GeometryNodeForm({
	node,
	nodes,
	onChange,
	onRemove,
}: {
	node: GeometryNode;
	nodes: GeometryNode[];
	onChange: (node: GeometryNode) => void;
	onRemove: () => void;
}) {
	const [tab, setTab] = useState<PartTab>("generic");
	const setTransform = (transform: Partial<GeometryNode["transform"]>) =>
		onChange({ ...node, transform: { ...node.transform, ...transform } });
	return (
		<div className="geometry-part-form">
			<h3>Part properties</h3>
			<div className="geometry-part-tabs" role="tablist" aria-label="Part properties">
				{TABS.map(([id, label]) => (
					<Button
						key={id}
						role="tab"
						aria-selected={tab === id}
						className={tab === id ? "is-active" : undefined}
						onClick={() => setTab(id)}
					>
						{label}
					</Button>
				))}
			</div>
			<div className="geometry-part-tab" role="tabpanel">
				{tab === "generic" && (
					<>
						<TextField
							label="Part name"
							value={node.name}
							onChange={(event) => onChange({ ...node, name: event.target.value })}
						/>
						<SelectField
							label="Parent part"
							value={node.parent_id ?? ""}
							options={[
								{ value: "", label: "Root" },
								...nodes
									.filter((candidate) => candidate.id !== node.id)
									.map((candidate) => ({
										value: candidate.id,
										label: candidate.name,
									})),
							]}
							onChange={(parent_id) =>
								onChange({ ...node, parent_id: parent_id || null })
							}
						/>
						<TextField
							label="GLB node binding"
							value={node.glb_node ?? ""}
							onChange={(event) =>
								onChange({ ...node, glb_node: event.target.value || null })
							}
						/>
						<Button
							variant="danger"
							disabled={nodes.length === 1}
							onClick={onRemove}
						>
							Remove part
						</Button>
					</>
				)}
				{tab === "translation" && (
					<VectorFields
						label="Translation (mm)"
						value={node.transform.translation}
						onChange={(translation) => setTransform({ translation })}
					/>
				)}
				{tab === "rotation" && (
					<VectorFields
						label="Base rotation °"
						value={node.transform.rotation_degrees}
						onChange={(rotation_degrees) => setTransform({ rotation_degrees })}
					/>
				)}
				{tab === "scale" && (
					<VectorFields
						label="Scale"
						value={node.transform.scale}
						onChange={(scale) => setTransform({ scale })}
					/>
				)}
				{tab === "pivot" && (
					<VectorFields
						label="Pivot (mm)"
						value={node.pivot}
						onChange={(pivot) => onChange({ ...node, pivot })}
					/>
				)}
				{tab === "animate" && (
					<>
						<CheckboxField
							label="Animated part"
							stateLabel="Moves with an attribute"
							checked={Boolean(node.motion)}
							onChange={(event) =>
								onChange({
									...node,
									motion: event.target.checked
										? {
												kind: "rotation",
												axis: { x: 0, y: 1, z: 0 },
												physical_min: -270,
												physical_max: 270,
											}
										: null,
								})
							}
						/>
						<p className="field-hint">
							Which attribute moves this part is chosen per mode, under{" "}
							<strong>Edit channels › Emitters &amp; Motion</strong>.
						</p>
						{node.motion && (
							<GeometryMotionFields
								motion={node.motion}
								onChange={(motion) => onChange({ ...node, motion })}
							/>
						)}
					</>
				)}
			</div>
		</div>
	);
}
