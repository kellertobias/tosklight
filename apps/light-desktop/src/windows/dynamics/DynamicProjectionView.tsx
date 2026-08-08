import { MultiValueToggleField, NumberField, RadioField } from "@tosklight/ui";
import { useEffect, useMemo, useRef, useState } from "react";
import type {
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
import { ProjectionStagePreview } from "../../features/spatialMapping/ProjectionStagePreview";
import type { StageFixtureDot } from "../../features/spatialMapping/stageFixtureDots";
import type {
	ProjectionKind,
	ProjectionPreset,
} from "../../features/spatialMapping/contracts";
import {
	PROJECTION_KINDS,
	PROJECTION_PRESETS,
	projectionFields,
	projectionKind,
	supportsPreset,
	withProjectionKind,
} from "../../features/spatialMapping/projectionKinds";
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
	const [applying, setApplying] = useState(false);
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

	// Changes persist as the operator makes them, so there is no Apply button and no
	// half-entered state to lose. A conflict reloads authority and keeps the draft.
	useEffect(() => {
		if (!dirty || validation != null || preview == null || busy) return;
		let cancelled = false;
		const timer = setTimeout(() => {
			setApplying(true);
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
				})
				.finally(() => {
					if (!cancelled) setApplying(false);
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

				<div className="dynamic-projection-settings">
					<h2>Projection</h2>
					<strong>{dynamicMappingBaseLabel(dynamic.body.target_binding)}</strong>
					{preview?.base.type === "live_group" && (
						<p className="dynamic-projection-help">
							Group mapping provenance:{" "}
							{provenanceLabel(preview.base.mapping_provenance)}
						</p>
					)}

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

					{validation && (
						<p className="dynamics-warning" role="alert">
							{validation}
						</p>
					)}
					<p className="dynamic-projection-message" role="status">
						{message ?? (applying ? "Saving…" : dirty ? "" : "Saved")}
					</p>
				</div>
			</section>
		</div>
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
		<>
			<div className="dynamic-projection-source">
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
			</div>
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
	return (
		<div className="dynamic-projection-fields">
			{/* The same toggle the Group's Projection uses, so the two read as one control. */}
			<MultiValueToggleField
				className="dynamic-projection-kinds"
				label="Projection"
				ariaLabel="Projection"
				value={kind}
				options={PROJECTION_KINDS.map(({ value: id, label }) => ({
					value: id,
					label,
				}))}
				onChange={(next) =>
					onChange(withProjectionKind(value, next as ProjectionKind))
				}
			/>
			{supportsPreset(value) && (
				<MultiValueToggleField
					className="dynamic-projection-presets"
					label="View preset"
					ariaLabel="View preset"
					value={value.preset ?? "custom"}
					options={[
						{ value: "custom", label: "Custom" },
						...PROJECTION_PRESETS.map(({ value: id, label }) => ({
							value: id,
							label,
						})),
					]}
					onChange={(preset) => {
						if (preset === "custom") onChange({ ...value, preset: null });
						else onChange(projectionForPreset(preset as ProjectionPreset));
					}}
				/>
			)}
			{projectionFields(value).map((field) => (
				<NumberField
					key={`${kind}-${field.key}`}
					label={field.label}
					value={field.value}
					unit={field.unit}
					allowDecimal
					showStepButtons={false}
					onValueChange={(next) => onChange(field.apply(Number(next)))}
				/>
			))}
			<p className="dynamic-projection-help">
				{PROJECTION_KINDS.find((entry) => entry.value === kind)?.detail}
			</p>
		</div>
	);
}
