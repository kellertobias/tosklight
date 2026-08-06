import { NumberField, RadioField, SelectField } from "@tosklight/ui";
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
import type { ProjectionKind } from "../../features/spatialMapping/contracts";
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

			{validation && (
				<p className="dynamics-warning" role="alert">
					{validation}
				</p>
			)}
			<p className="dynamic-projection-message" role="status">
				{message ?? (applying ? "Saving…" : dirty ? "" : "Saved")}
			</p>
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
				<div className="dynamic-projection-columns">
					<div className="dynamic-projection-visual">
						<ProjectionStagePreview projection={draft.projection.value} />
						<small>Drag to orbit</small>
					</div>
					<ProjectionFields value={draft.projection.value} onChange={onChange} />
				</div>
			) : (
				<p className="dynamic-projection-help">
					Uses the projection inherited from the Group.
				</p>
			)}
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
	const kind = projectionKind(value);
	return (
		<div className="dynamic-projection-fields">
			<SelectField
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
				<SelectField
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
						else onChange(projectionForPreset(preset));
					}}
				/>
			)}
			{projectionFields(value).map((field) => (
				<NumberField
					key={field.key}
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
