import {
	Button,
	MultiValueToggleField,
	NumberField as UiNumberField,
} from "@tosklight/ui";
import { useEffect, useMemo, useRef, useState } from "react";
import type {
	DynamicSpatialMappingOverrideProjection,
	DynamicSpatialPreviewResponse,
	GroupMappingProvenanceProjection,
} from "../../api/types";
import {
	dynamicSpatialDraft,
	sameDynamicSpatialDraft,
	validateDynamicSpatialDraft,
} from "../../features/dynamics/dynamicSpatialDraft";
import type { ShowObject } from "../../features/showObjects/contracts";
import type {
	ProjectionKind,
	ProjectionPreset,
} from "../../features/spatialMapping/contracts";
import { ProjectionStagePreview } from "../../features/spatialMapping/ProjectionStagePreview";
import {
	PROJECTION_KINDS,
	PROJECTION_PRESETS,
	projectionFields,
	projectionKind,
	supportsPreset,
	withProjectionKind,
} from "../../features/spatialMapping/projectionKinds";
import type { StageFixtureDot } from "../../features/spatialMapping/stageFixtureDots";
import { projectionForPreset } from "../groupsWindow/spatialMapping";

/** Long enough that typing a multi-digit value is one save, short enough to feel immediate. */
const APPLY_DEBOUNCE_MS = 400;

type DynamicObject = ShowObject<"dynamic">;
export type DynamicSpatialApplyResult = "applied" | "conflict";

interface DynamicProjectionViewProps {
	dynamic: DynamicObject;
	busy: boolean;
	/** Lamp positions drawn under the shape, so it can be placed against the real rig. */
	fixtures?: readonly StageFixtureDot[];
	loadPreview(
		draft: DynamicSpatialMappingOverrideProjection,
	): Promise<DynamicSpatialPreviewResponse>;
	apply(
		draft: DynamicSpatialMappingOverrideProjection,
	): Promise<DynamicSpatialApplyResult>;
}

const TOP_PROJECTION = projectionForPreset("top");

function DynamicProjectionSettings({
	preview,
	draft,
	validation,
	message,
	onModeChange,
	onDraft,
}: {
	preview: DynamicSpatialPreviewResponse | null;
	draft: ReturnType<typeof dynamicSpatialDraft>;
	validation: string | null;
	message: string | null;
	onModeChange(mode: "inherit" | ProjectionKind): void;
	onDraft(draft: ReturnType<typeof dynamicSpatialDraft>): void;
}) {
	return (
		<div className="dynamic-projection-settings">
			{preview?.base.type === "live_group" && (
				<p className="dynamic-projection-help">
					Group mapping provenance:{" "}
					{provenanceLabel(preview.base.mapping_provenance)}
				</p>
			)}
			<ProjectionControls
				draft={draft}
				onModeChange={onModeChange}
				onChange={(value) =>
					onDraft({ ...draft, projection: { type: "replace", value } })
				}
			/>
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
		</div>
	);
}

export function DynamicProjectionView({
	dynamic,
	busy,
	fixtures,
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
	const [message, setMessage] = useState<string | null>(null);
	const request = useRef(0);
	const previousSource = useRef<string | null>(null);
	const lastSaved = useRef(saved);

	useEffect(() => {
		setDraft(saved);
		lastSaved.current = saved;
		setMessage(null);
	}, [dynamic.id]);

	// Encoders write the same projection through the Dynamic itself, so authority can move while
	// this view is open. Without adopting it the draft stays on the old numbers, the picture stops
	// following the encoders, and the auto-apply below writes the stale draft straight back over
	// them.
	useEffect(() => {
		if (sameDynamicSpatialDraft(lastSaved.current, saved)) return;
		lastSaved.current = saved;
		setDraft(saved);
	}, [saved]);

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
	const validation = validateDynamicSpatialDraft(draft);
	const dirty = !sameDynamicSpatialDraft(draft, saved);
	const changeProjectionMode = (mode: "inherit" | ProjectionKind) =>
		setDraft((current) => {
			if (mode === "inherit")
				return { ...current, projection: { type: "inherit" } };
			const base =
				current.projection.type === "replace"
					? current.projection.value
					: (inherited?.projection ?? TOP_PROJECTION);
			return {
				...current,
				projection: {
					type: "replace",
					value: withProjectionKind(base, mode),
				},
			};
		});

	// Changes persist as the operator makes them, so there is no Apply button and no
	// half-entered state to lose. A conflict reloads authority and keeps the draft.
	useEffect(() => {
		if (!dirty || validation != null || preview == null || busy) return;
		let cancelled = false;
		const timer = setTimeout(() => {
			apply(draft)
				.then((result) => {
					if (cancelled) return;
					setMessage(
						result === "conflict"
							? "The Dynamic changed elsewhere. Authoritative values were reloaded; your change is retained."
							: null,
					);
				})
				.catch((error: unknown) => {
					if (cancelled) return;
					setMessage(error instanceof Error ? error.message : String(error));
				});
		}, APPLY_DEBOUNCE_MS);
		return () => {
			cancelled = true;
			clearTimeout(timer);
		};
	}, [draft, dirty, validation, preview, busy, apply]);

	// The projection being shown: the operator's override, or the Group's while inheriting,
	// so the left column always depicts what this Dynamic actually ranks by.
	const shown =
		draft.projection.type === "replace"
			? draft.projection.value
			: (inherited?.projection ?? null);

	return (
		<div className="dynamic-projection-view">
			<section className="dynamic-projection-card dynamic-projection-columns">
				<div className="dynamic-projection-visual">
					{shown ? (
						<>
							<ProjectionStagePreview projection={shown} fixtures={fixtures} />
							<small>Drag to orbit</small>
						</>
					) : (
						<p className="dynamic-projection-help">
							No projection to show. Targets keep their saved selection order.
						</p>
					)}
				</div>

				<DynamicProjectionSettings
					preview={preview}
					draft={draft}
					validation={validation}
					message={message}
					onModeChange={changeProjectionMode}
					onDraft={setDraft}
				/>
			</section>
		</div>
	);
}

function ProjectionControls({
	draft,
	onModeChange,
	onChange,
}: {
	draft: ReturnType<typeof dynamicSpatialDraft>;
	onModeChange(mode: "inherit" | ProjectionKind): void;
	onChange(value: typeof TOP_PROJECTION): void;
}) {
	const mode =
		draft.projection.type === "inherit"
			? "inherit"
			: projectionKind(draft.projection.value);
	return (
		<>
			<MultiValueToggleField
				className="group-projection-kinds dynamic-projection-kinds"
				label="Projection"
				ariaLabel="Projection"
				value={mode}
				options={[
					{ value: "inherit", label: "Inherit" },
					...PROJECTION_KINDS.map(({ value, label }) => ({ value, label })),
				]}
				onChange={(next) => onModeChange(next as "inherit" | ProjectionKind)}
			/>
			{draft.projection.type === "replace" ? (
				<ProjectionFields value={draft.projection.value} onChange={onChange} />
			) : (
				<p className="dynamic-projection-help">
					Uses the projection inherited from the Group.
				</p>
			)}
		</>
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
	const kind = projectionKind(value);
	const fields = projectionFields(value);
	const positionFields = fields.filter((field) =>
		field.key.startsWith("position-"),
	);
	const directionFields = fields.filter(
		(field) => !field.key.startsWith("position-"),
	);
	return (
		<fieldset className="group-mapping-fields group-projection-fields dynamic-projection-fields">
			<legend>Projection settings</legend>
			<div className="group-vector-fields">
				{supportsPreset(value) ? (
					<fieldset className="group-vector-column group-preset-fields">
						<legend>Presets</legend>
						<div className="group-projection-presets">
							{PROJECTION_PRESETS.map(({ value: preset, label }) => (
								<Button
									key={preset}
									aria-pressed={value.preset === preset}
									onClick={() =>
										onChange({
											...projectionForPreset(preset as ProjectionPreset),
											anchor: value.anchor,
										})
									}
								>
									{label}
								</Button>
							))}
						</div>
					</fieldset>
				) : positionFields.length > 0 ? (
					<fieldset className="group-vector-column group-position-fields">
						<legend>Position</legend>
						{positionFields.map((field) => (
							<ProjectionNumberField
								key={`${kind}-${field.key}`}
								label={field.label}
								value={field.value}
								unit={field.unit}
								onCommit={(next) => onChange(field.apply(next))}
							/>
						))}
					</fieldset>
				) : null}
				<fieldset className="group-vector-column group-direction-fields">
					<legend>Direction</legend>
					{directionFields.map((field) => (
						<ProjectionNumberField
							key={`${kind}-${field.key}`}
							label={field.label}
							value={field.value}
							unit={field.unit}
							angle={field.unit === "°"}
							onCommit={(next) => onChange(field.apply(next))}
						/>
					))}
				</fieldset>
			</div>
			<p className="dynamic-projection-help">
				{PROJECTION_KINDS.find((entry) => entry.value === kind)?.detail}
			</p>
		</fieldset>
	);
}

function ProjectionNumberField({
	label,
	value,
	unit,
	angle = false,
	onCommit,
}: {
	label: string;
	value: number;
	unit?: string;
	angle?: boolean;
	onCommit(value: number): void;
}) {
	return (
		<UiNumberField
			className="group-number-input"
			label={label}
			unit={unit}
			defaultValue={value}
			allowDecimal
			showStepButtons={angle}
			step={angle ? 45 : undefined}
			stepBehavior={angle ? "snap" : undefined}
			wrapStepAtBounds={angle}
			min={angle ? -180 : undefined}
			max={angle ? 180 : undefined}
			onStepCommit={(next) => onCommit(Number(next))}
			onKeyboardCommit={(next) => onCommit(Number(next))}
			onBlur={(event) => onCommit(Number(event.currentTarget.value))}
			onKeyDown={(event) => {
				if (event.key === "Enter") event.currentTarget.blur();
			}}
		/>
	);
}
