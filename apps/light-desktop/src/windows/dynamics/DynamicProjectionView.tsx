import { Button, NumberField, RadioField, SelectField } from "@tosklight/ui";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type {
	DynamicSelectionShapeProjection,
	DynamicSpatialMappingOverrideProjection,
	DynamicSpatialPreviewResponse,
	GroupMappingProvenanceProjection,
} from "../../api/types";
import {
	dynamicMappingBaseLabel,
	dynamicSpatialDraft,
	sameDynamicSpatialDraft,
	validateDynamicSpatialDraft,
} from "../../features/dynamics/dynamicSpatialDraft";
import type { ShowObject } from "../../features/showObjects/contracts";
import { projectionForPreset } from "../groupsWindow/spatialMapping";

type DynamicObject = ShowObject<"dynamic">;
export type DynamicSpatialApplyResult = "applied" | "conflict";

interface DynamicProjectionViewProps {
	dynamic: DynamicObject;
	busy: boolean;
	loadPreview(
		draft: DynamicSpatialMappingOverrideProjection,
	): Promise<DynamicSpatialPreviewResponse>;
	apply(
		draft: DynamicSpatialMappingOverrideProjection,
	): Promise<DynamicSpatialApplyResult>;
}

const TOP_PROJECTION = projectionForPreset("top");
const GRID_SHAPE: DynamicSelectionShapeProjection = {
	type: "grid",
	angle_degrees: 0,
	direction: "ascending",
};

export function DynamicProjectionView({
	dynamic,
	busy,
	loadPreview,
	apply,
}: DynamicProjectionViewProps) {
	const saved = useMemo(
		() => dynamicSpatialDraft(dynamic.body.spatial_mapping),
		[dynamic.id, dynamic.revision, dynamic.body.spatial_mapping],
	);
	const [draft, setDraft] = useState(saved);
	const [preview, setPreview] = useState<DynamicSpatialPreviewResponse | null>(
		null,
	);
	const [loading, setLoading] = useState(true);
	const [applying, setApplying] = useState(false);
	const [message, setMessage] = useState<string | null>(null);
	const request = useRef(0);
	const previousSource = useRef<string | null>(null);

	useEffect(() => {
		setDraft(saved);
		setMessage(null);
	}, [dynamic.id]);

	useEffect(() => {
		const sequence = ++request.current;
		setLoading(true);
		void loadPreview(draft)
			.then((next) => {
				if (request.current !== sequence) return;
				const signature = JSON.stringify({
					base: next.base,
					inherited: next.inherited_mapping,
					source: next.source_order,
					ordered: next.ordered_fixture_ids,
					ranks: next.ranks,
				});
				if (
					previousSource.current != null &&
					previousSource.current !== signature &&
					!sameDynamicSpatialDraft(draft, saved)
				)
					setMessage(
						"Source changed. Preview refreshed; your draft was retained.",
					);
				previousSource.current = signature;
				setPreview(next);
				setLoading(false);
			})
			.catch((error: unknown) => {
				if (request.current !== sequence) return;
				setLoading(false);
				setMessage(error instanceof Error ? error.message : String(error));
			});
	}, [draft, loadPreview, saved]);

	const inherited = preview?.inherited_mapping ?? null;
	const validation = validateDynamicSpatialDraft(draft, inherited != null);
	const dirty = !sameDynamicSpatialDraft(draft, saved);
	const changeProjection = (override: boolean) =>
		setDraft((current) => ({
			...current,
			projection: override
				? {
						type: "replace",
						value: inherited?.projection ?? TOP_PROJECTION,
					}
				: { type: "inherit" },
		}));
	const changeShape = (override: boolean) =>
		setDraft((current) => ({
			...current,
			shape: override
				? { type: "replace", value: inherited?.shape ?? GRID_SHAPE }
				: { type: "inherit" },
		}));
	const applyDraft = useCallback(async () => {
		setMessage(null);
		setApplying(true);
		try {
			const result = await apply(draft);
			if (result === "conflict") {
				setMessage(
					"The Dynamic changed elsewhere. Authoritative values were reloaded; your draft is retained. Review it and Apply again.",
				);
				return;
			}
			setMessage("Spatial mapping applied.");
		} catch (error) {
			setMessage(error instanceof Error ? error.message : String(error));
		} finally {
			setApplying(false);
		}
	}, [apply, draft]);

	return (
		<div className="dynamic-projection-view">
			<ProjectionHeader dynamic={dynamic} preview={preview} />

			<ProjectionControls
				draft={draft}
				onOverride={changeProjection}
				onChange={(value) =>
					setDraft((current) => ({
						...current,
						projection: { type: "replace", value },
					}))
				}
			/>
			<ShapeControls
				draft={draft}
				onOverride={changeShape}
				onChange={(value) =>
					setDraft((current) => ({
						...current,
						shape: { type: "replace", value },
					}))
				}
			/>
			<ProjectionPreview loading={loading} preview={preview} />

			{validation && (
				<p className="dynamics-warning" role="alert">
					{validation}
				</p>
			)}
			{message && (
				<p className="dynamic-projection-message" role="status">
					{message}
				</p>
			)}
			<Button
				disabled={
					busy ||
					applying ||
					loading ||
					!dirty ||
					validation != null ||
					preview == null
				}
				onClick={() => void applyDraft()}
			>
				Apply
			</Button>
		</div>
	);
}

function ProjectionHeader({
	dynamic,
	preview,
}: {
	dynamic: DynamicObject;
	preview: DynamicSpatialPreviewResponse | null;
}) {
	return (
		<section className="dynamic-projection-card">
			<h2>Projection</h2>
			<strong>{dynamicMappingBaseLabel(dynamic.body.target_binding)}</strong>
			{preview?.base.type === "live_group" && (
				<p>
					Group mapping provenance:{" "}
					{provenanceLabel(preview.base.mapping_provenance)}
				</p>
			)}
			<p className="dynamic-projection-help">
				Preview uses this Dynamic’s saved target binding, never the current
				Programmer selection.
			</p>
		</section>
	);
}

function ProjectionControls({
	draft,
	onOverride,
	onChange,
}: {
	draft: ReturnType<typeof dynamicSpatialDraft>;
	onOverride(override: boolean): void;
	onChange(value: typeof TOP_PROJECTION): void;
}) {
	return (
		<section className="dynamic-projection-card">
			<h3>Projection stage</h3>
			<RadioField
				label="Inherit"
				stateLabel="Inherit"
				name="dynamic-projection-source"
				checked={draft.projection.type === "inherit"}
				onChange={() => onOverride(false)}
			/>
			<RadioField
				label="Override"
				stateLabel="Override"
				name="dynamic-projection-source"
				checked={draft.projection.type === "replace"}
				onChange={() => onOverride(true)}
			/>
			{draft.projection.type === "replace" && (
				<ProjectionFields value={draft.projection.value} onChange={onChange} />
			)}
		</section>
	);
}

function ShapeControls({
	draft,
	onOverride,
	onChange,
}: {
	draft: ReturnType<typeof dynamicSpatialDraft>;
	onOverride(override: boolean): void;
	onChange(value: DynamicSelectionShapeProjection): void;
}) {
	return (
		<section className="dynamic-projection-card">
			<h3>Phaser shape</h3>
			<RadioField
				label="Inherit"
				stateLabel="Inherit"
				name="dynamic-shape-source"
				checked={draft.shape.type === "inherit"}
				onChange={() => onOverride(false)}
			/>
			<RadioField
				label="Override"
				stateLabel="Override"
				name="dynamic-shape-source"
				checked={draft.shape.type === "replace"}
				onChange={() => onOverride(true)}
			/>
			{draft.shape.type === "replace" && (
				<ShapeFields value={draft.shape.value} onChange={onChange} />
			)}
		</section>
	);
}

function ProjectionPreview({
	loading,
	preview,
}: {
	loading: boolean;
	preview: DynamicSpatialPreviewResponse | null;
}) {
	return (
		<section className="dynamic-projection-card dynamic-projection-preview">
			<h3>Authoritative preview</h3>
			{loading ? (
				<p>Refreshing preview…</p>
			) : preview?.ordered_fixture_ids.length ? (
				<ol>
					{preview.ordered_fixture_ids.map((id, index) => (
						<li key={id}>
							<code>{id}</code>
							<span>rank {preview.ranks[index]?.rank ?? index + 1}</span>
						</li>
					))}
				</ol>
			) : (
				<p>No saved targets to rank.</p>
			)}
			{preview?.warnings.map((warning, index) => (
				<p className="dynamics-warning" key={`${warning.fixture_id}-${index}`}>
					Fixture {warning.fixture_id} has no Stage position and follows saved
					selection order.
				</p>
			))}
		</section>
	);
}

function provenanceLabel(provenance: GroupMappingProvenanceProjection) {
	switch (provenance.type) {
		case "local":
			return `local Group ${provenance.group_id}`;
		case "inherited":
			return `inherited from Group ${provenance.source_group_ids.join(", ")}`;
		case "mixed_source_mappings":
			return "mixed source Group mappings";
		case "none":
			return "none";
	}
}

function ProjectionFields({
	value,
	onChange,
}: {
	value: typeof TOP_PROJECTION;
	onChange(value: typeof TOP_PROJECTION): void;
}) {
	const number = (
		label: string,
		current: number,
		change: (next: number) => typeof TOP_PROJECTION,
	) => (
		<NumberField
			label={label}
			value={current}
			allowDecimal
			showStepButtons={false}
			onValueChange={(next) => onChange(change(Number(next)))}
		/>
	);
	return (
		<div className="dynamic-projection-fields">
			<SelectField
				label="View preset"
				ariaLabel="View preset"
				value={value.preset ?? "custom"}
				options={[
					{ value: "custom", label: "Custom" },
					{ value: "top", label: "Top" },
					{ value: "front", label: "Front" },
					{ value: "back", label: "Back" },
					{ value: "left", label: "Left" },
					{ value: "right", label: "Right" },
				]}
				onChange={(preset) => {
					if (preset === "custom") onChange({ ...value, preset: null });
					else onChange(projectionForPreset(preset));
				}}
			/>
			{number("Anchor X", value.anchor.x, (x) => ({
				...value,
				anchor: { ...value.anchor, x },
				preset: null,
			}))}
			{number("Anchor Y", value.anchor.y, (y) => ({
				...value,
				anchor: { ...value.anchor, y },
				preset: null,
			}))}
			{number("Anchor Z", value.anchor.z, (z) => ({
				...value,
				anchor: { ...value.anchor, z },
				preset: null,
			}))}
			{number("Direction X", value.view_direction.x, (x) => ({
				...value,
				view_direction: { ...value.view_direction, x },
				preset: null,
			}))}
			{number("Direction Y", value.view_direction.y, (y) => ({
				...value,
				view_direction: { ...value.view_direction, y },
				preset: null,
			}))}
			{number("Direction Z", value.view_direction.z, (z) => ({
				...value,
				view_direction: { ...value.view_direction, z },
				preset: null,
			}))}
			{number("Rotation", value.rotation_degrees, (rotation_degrees) => ({
				...value,
				rotation_degrees,
				preset: null,
			}))}
		</div>
	);
}

function ShapeFields({
	value,
	onChange,
}: {
	value: DynamicSelectionShapeProjection;
	onChange(value: DynamicSelectionShapeProjection): void;
}) {
	return (
		<div className="dynamic-projection-fields">
			<SelectField
				label="Shape"
				ariaLabel="Shape"
				value={value.type}
				options={[
					{ value: "grid", label: "Grid" },
					{ value: "radial", label: "Radial" },
					{ value: "radar", label: "Radar" },
					{ value: "random", label: "Random" },
				]}
				onChange={(type) => {
					onChange(
						type === "random"
							? { type, seed: 0 }
							: type === "radial"
								? { type, center_u: 0.5, center_v: 0.5, direction: "outward" }
								: type === "radar"
									? {
											type,
											center_u: 0.5,
											center_v: 0.5,
											start_angle_degrees: 0,
											sweep: "clockwise",
										}
									: GRID_SHAPE,
					);
				}}
			/>
			{value.type === "random" && (
				<>
					<NumberField
						label="Seed"
						min={0}
						value={value.seed}
						onValueChange={(seed) => onChange({ ...value, seed: Number(seed) })}
					/>
					<p>Random ignores fixture positions and the Projection stage.</p>
				</>
			)}
			{value.type === "grid" && (
				<>
					<ShapeNumber
						label="Angle"
						value={value.angle_degrees}
						onChange={(angle_degrees) => onChange({ ...value, angle_degrees })}
					/>
					<SelectField
						label="Direction"
						ariaLabel="Direction"
						value={value.direction}
						options={[
							{ value: "ascending", label: "Ascending" },
							{ value: "descending", label: "Descending" },
						]}
						onChange={(direction) => onChange({ ...value, direction })}
					/>
				</>
			)}
			{value.type === "radial" && (
				<>
					<ShapeNumber
						label="Center U"
						value={value.center_u}
						onChange={(center_u) => onChange({ ...value, center_u })}
					/>
					<ShapeNumber
						label="Center V"
						value={value.center_v}
						onChange={(center_v) => onChange({ ...value, center_v })}
					/>
					<SelectField
						label="Direction"
						ariaLabel="Direction"
						value={value.direction}
						options={[
							{ value: "outward", label: "Outward" },
							{ value: "inward", label: "Inward" },
						]}
						onChange={(direction) => onChange({ ...value, direction })}
					/>
				</>
			)}
			{value.type === "radar" && (
				<>
					<ShapeNumber
						label="Center U"
						value={value.center_u}
						onChange={(center_u) => onChange({ ...value, center_u })}
					/>
					<ShapeNumber
						label="Center V"
						value={value.center_v}
						onChange={(center_v) => onChange({ ...value, center_v })}
					/>
					<ShapeNumber
						label="Start angle"
						value={value.start_angle_degrees}
						onChange={(start_angle_degrees) =>
							onChange({ ...value, start_angle_degrees })
						}
					/>
					<SelectField
						label="Sweep"
						ariaLabel="Sweep"
						value={value.sweep}
						options={[
							{ value: "clockwise", label: "Clockwise" },
							{
								value: "counter_clockwise",
								label: "Counter-clockwise",
							},
						]}
						onChange={(sweep) => onChange({ ...value, sweep })}
					/>
				</>
			)}
		</div>
	);
}

function ShapeNumber({
	label,
	value,
	onChange,
}: {
	label: string;
	value: number;
	onChange(value: number): void;
}) {
	return (
		<NumberField
			label={label}
			value={value}
			allowDecimal
			showStepButtons={false}
			onValueChange={(next) => onChange(Number(next))}
		/>
	);
}
