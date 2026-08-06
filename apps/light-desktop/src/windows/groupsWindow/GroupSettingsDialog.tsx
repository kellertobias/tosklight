import {
	Button,
	ColorPickerField,
	FormLayout,
	MultiValueToggleField,
	IconPickerField,
	TextField,
	NumberField as UiNumberField,
	SelectField as UiSelectField,
} from "@tosklight/ui";
import { WindowSettings } from "@tosklight/ui/window-kit";
import { useCallback, useEffect, useState } from "react";
import { useGroupManagement } from "../../features/groupManagement/GroupManagementProvider";
import type { ResolvedSpatialMapping } from "../../features/spatialMapping/contracts";
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
import type { Group } from "./model";
import {
	defaultSpatialMapping,
	groupSourceSummary,
	hasGroupReferenceSource,
	projectionForPreset,
	resolveMappingPresentation,
	type SpatialSelectionMapping,
	type SpatialSelectionShape,
	storedMapping,
	validateSpatialMapping,
} from "./spatialMapping";

export function GroupSettingsDialog({
	group,
	groups,
	onClose,
}: {
	group: Group;
	groups: readonly Group[];
	onClose: () => void;
}) {
	const [name, setName] = useState(group.body.name ?? `Group ${group.id}`);
	const [color, setColor] = useState(group.body.color ?? "#718596");
	const [icon, setIcon] = useState(group.body.icon ?? "◇");
	const [status, setStatus] = useState<string | null>(null);
	const [saving, setSaving] = useState(false);
	const {
		groupManagement,
		expectedRevision,
		setExpectedRevision,
		expectedShowRevision,
		setExpectedShowRevision,
		mappingPresentation,
		setMappingPresentation,
		mapping,
		setMapping,
		resolvedSpatial,
		refreshSettings,
	} = useGroupSettingsAuthority(group, groups);

	const saveProperties = async (next: {
		name: string;
		color: string;
		icon: string;
	}) => {
		const trimmed = next.name.trim();
		if (!trimmed) return setStatus("Group name must not be empty.");
		if (!groupManagement || saving) return;
		setSaving(true);
		setStatus("Saving Group settings…");
		const authority =
			expectedShowRevision == null ? await refreshSettings() : null;
		const objectRevision = authority?.group.revision ?? expectedRevision;
		const showRevision = authority?.showRevision ?? expectedShowRevision;
		if (showRevision == null) {
			setSaving(false);
			setStatus("Authoritative Group settings are still loading.");
			return;
		}
		const outcome = await groupManagement.manage({
			objectId: group.id,
			expectedObjectRevision: objectRevision,
			expectedShowRevision: showRevision,
			operation: {
				type: "update_properties",
				properties: { name: trimmed, color: next.color, icon: next.icon },
			},
		});
		setSaving(false);
		if (!outcome) {
			await refreshSettings();
			setStatus(
				"Change was not accepted. Authoritative Group values were reloaded; review and retry.",
			);
			return;
		}
		setExpectedRevision(outcome.group.revision);
		setExpectedShowRevision(outcome.showRevision);
		setStatus(outcome.persistenceWarning ?? "Saved.");
	};

	const commitMapping = async (next: SpatialSelectionMapping | null) => {
		if (!groupManagement || saving) return;
		const validation = next ? validateSpatialMapping(next) : null;
		if (validation) return setStatus(validation);
		setSaving(true);
		setStatus("Saving spatial mapping…");
		const authority =
			expectedShowRevision == null ? await refreshSettings() : null;
		const objectRevision = authority?.group.revision ?? expectedRevision;
		const showRevision = authority?.showRevision ?? expectedShowRevision;
		if (showRevision == null) {
			setSaving(false);
			setStatus("Authoritative Group settings are still loading.");
			return;
		}
		const outcome = await groupManagement.manage({
			objectId: group.id,
			expectedObjectRevision: objectRevision,
			expectedShowRevision: showRevision,
			operation: next
				? { type: "set_spatial_mapping", mapping: next }
				: { type: "remove_spatial_mapping" },
		});
		setSaving(false);
		if (!outcome) {
			await refreshSettings();
			setStatus(
				"Mapping change was not accepted. Reloaded authoritative values must be reviewed before retrying.",
			);
			return;
		}
		setExpectedRevision(outcome.group.revision);
		setExpectedShowRevision(outcome.showRevision);
		setMapping(next);
		setMappingPresentation(
			next
				? { type: "local", label: "Local override", mapping: next }
				: resolveMappingPresentation(
						{ ...group, body: { ...group.body, mapping: undefined } } as Group,
						groups,
					),
		);
		await refreshSettings();
		setStatus("Saved.");
	};

	const editingUnavailable = !groupManagement
		? "Spatial mapping editing is unavailable until the revisioned Group mapping action is connected."
		: null;
	const displayedMapping = mapping ?? mappingPresentation.mapping;
	const canEditMapping = Boolean(groupManagement && mapping);
	const commonPanelProps = {
		group,
		mappingPresentation,
		displayedMapping,
		resolvedSpatial,
		status,
		saving,
		canEditMapping,
		commitMapping,
	};

	return (
		<GroupSettingsWindow
			group={group}
			onClose={onClose}
			name={name}
			color={color}
			icon={icon}
			saving={saving}
			status={status}
			setName={setName}
			setColor={setColor}
			setIcon={setIcon}
			saveProperties={saveProperties}
			commonPanelProps={commonPanelProps}
			editingUnavailable={editingUnavailable}
			hasManagement={Boolean(groupManagement)}
		/>
	);
}

function useGroupSettingsAuthority(group: Group, groups: readonly Group[]) {
	const groupManagement = useGroupManagement();
	const [expectedRevision, setExpectedRevision] = useState(group.revision);
	const [expectedShowRevision, setExpectedShowRevision] = useState<
		number | null
	>(null);
	const [mappingPresentation, setMappingPresentation] = useState(() =>
		resolveMappingPresentation(group, groups),
	);
	const [mapping, setMapping] = useState<SpatialSelectionMapping | null>(() =>
		mappingPresentation.type === "local" ? mappingPresentation.mapping : null,
	);
	const [resolvedSpatial, setResolvedSpatial] =
		useState<ResolvedSpatialMapping | null>(null);
	const refreshSettings = useCallback(async () => {
		if (!groupManagement) return null;
		const snapshot = await groupManagement.settings(group.id);
		if (!snapshot) return null;
		setExpectedRevision(snapshot.group.revision);
		setExpectedShowRevision(snapshot.showRevision);
		setResolvedSpatial(snapshot.resolvedSpatial);
		setMappingPresentation(
			resolvedMappingPresentation(snapshot.resolvedSpatial),
		);
		setMapping(
			snapshot.resolvedSpatial.mapping_provenance.type === "local"
				? storedMapping(snapshot.group.object.body)
				: null,
		);
		return snapshot;
	}, [group.id, groupManagement]);
	useEffect(() => {
		void refreshSettings();
	}, [refreshSettings]);
	return {
		groupManagement,
		expectedRevision,
		setExpectedRevision,
		expectedShowRevision,
		setExpectedShowRevision,
		mappingPresentation,
		setMappingPresentation,
		mapping,
		setMapping,
		resolvedSpatial,
		refreshSettings,
	};
}

function GroupSettingsWindow({
	group,
	onClose,
	name,
	color,
	icon,
	saving,
	status,
	setName,
	setColor,
	setIcon,
	saveProperties,
	commonPanelProps,
	editingUnavailable,
	hasManagement,
}: {
	group: Group;
	onClose(): void;
	name: string;
	color: string;
	icon: string;
	saving: boolean;
	status: string | null;
	setName(value: string): void;
	setColor(value: string): void;
	setIcon(value: string): void;
	saveProperties(next: {
		name: string;
		color: string;
		icon: string;
	}): Promise<void>;
	commonPanelProps: MappingPanelProps;
	editingUnavailable: string | null;
	hasManagement: boolean;
}) {
	return (
		<WindowSettings
			title={`Group ${group.id} settings`}
			onClose={onClose}
			tabs={[
				{
					id: "general",
					label: "General",
					content: (
						<GeneralSettingsPanel
							name={name}
							color={color}
							icon={icon}
							saving={saving}
							status={status}
							setName={setName}
							setColor={setColor}
							setIcon={setIcon}
							save={saveProperties}
						/>
					),
				},
				{
					id: "projection",
					label: "Projection",
					content: (
						<ProjectionSettingsPanel
							{...commonPanelProps}
							editingUnavailable={editingUnavailable}
							hasManagement={hasManagement}
						/>
					),
				},
				{
					id: "phase",
					label: "Phase",
					content: <PhaseSettingsPanel {...commonPanelProps} />,
				},
			]}
		/>
	);
}

function GeneralSettingsPanel({
	name,
	color,
	icon,
	saving,
	status,
	setName,
	setColor,
	setIcon,
	save,
}: {
	name: string;
	color: string;
	icon: string;
	saving: boolean;
	status: string | null;
	setName(value: string): void;
	setColor(value: string): void;
	setIcon(value: string): void;
	save(next: { name: string; color: string; icon: string }): Promise<void>;
}) {
	return (
		<section className="group-settings-panel group-settings-general">
			<fieldset disabled={saving} className="group-general-fields">
				<FormLayout labelPlacement="side">
					<TextField
						label="Group name"
						clearable
						autoFocus
						value={name}
						onChange={(event) => setName(event.target.value)}
						onBlur={() => void save({ name, color, icon })}
						onKeyDown={(event) => {
							if (event.key === "Enter") event.currentTarget.blur();
						}}
					/>
					<ColorPickerField
						label="Color"
						value={color}
						onChange={(next) => {
							setColor(next);
							void save({ name, color: next, icon });
						}}
					/>
					<IconPickerField
						label="Icon"
						value={icon}
						onChange={(next) => {
							setIcon(next);
							void save({ name, color, icon: next });
						}}
					/>
				</FormLayout>
			</fieldset>
			<SaveStatus status={status} saving={saving} />
		</section>
	);
}

interface MappingPanelProps {
	group: Group;
	mappingPresentation: ReturnType<typeof resolveMappingPresentation>;
	displayedMapping: SpatialSelectionMapping | null;
	resolvedSpatial: ResolvedSpatialMapping | null;
	status: string | null;
	saving: boolean;
	canEditMapping: boolean;
	commitMapping(next: SpatialSelectionMapping | null): Promise<void>;
}

function ProjectionSettingsPanel(
	props: MappingPanelProps & {
		editingUnavailable: string | null;
		hasManagement: boolean;
	},
) {
	const {
		group,
		mappingPresentation,
		displayedMapping,
		status,
		saving,
		canEditMapping,
		commitMapping,
		editingUnavailable,
		hasManagement,
	} = props;
	return (
		<section className="group-settings-panel">
			<MappingIdentity
				label={mappingPresentation.label}
				source={groupSourceSummary(group.body)}
			/>
			<MappingOwnershipActions
				presentation={mappingPresentation.type}
				hasInheritedSource={hasGroupReferenceSource(group.body)}
				disabled={!hasManagement || saving}
				onCreate={() => void commitMapping(defaultSpatialMapping())}
				onCopy={() =>
					mappingPresentation.mapping &&
					void commitMapping(structuredClone(mappingPresentation.mapping))
				}
				onRemove={() => void commitMapping(null)}
			/>
			{editingUnavailable && <p role="note">{editingUnavailable}</p>}
			{displayedMapping ? (
				<ProjectionEditor
					mapping={displayedMapping}
					disabled={!canEditMapping || saving}
					onChange={(next) => void commitMapping(next)}
				/>
			) : (
				<p className="group-mapping-empty">
					Source order is used until this Group has an inherited or local
					mapping.
				</p>
			)}
			<SaveStatus status={status} saving={saving} />
		</section>
	);
}

function PhaseSettingsPanel({
	group,
	mappingPresentation,
	displayedMapping,
	resolvedSpatial,
	status,
	saving,
	canEditMapping,
	commitMapping,
}: MappingPanelProps) {
	return (
		<section className="group-settings-panel">
			<MappingIdentity
				label={mappingPresentation.label}
				source={groupSourceSummary(group.body)}
			/>
			{displayedMapping ? (
				<PhaseEditor
					mapping={displayedMapping}
					disabled={!canEditMapping || saving}
					onChange={(next) => void commitMapping(next)}
				/>
			) : (
				<p className="group-mapping-empty">No Phase mapping is configured.</p>
			)}
			<MappingPreview
				title="Ranked preview"
				fixtures={group.body.fixtures.length}
				resolved={resolvedSpatial}
			/>
			<SaveStatus status={status} saving={saving} />
		</section>
	);
}

function MappingIdentity({ label, source }: { label: string; source: string }) {
	return (
		<div className="group-mapping-identity" aria-live="polite">
			<strong>{label}</strong>
			<span>{source}</span>
		</div>
	);
}

function MappingOwnershipActions({
	presentation,
	hasInheritedSource,
	disabled,
	onCreate,
	onCopy,
	onRemove,
}: {
	presentation: "none" | "local" | "inherited" | "mixed";
	hasInheritedSource: boolean;
	disabled: boolean;
	onCreate: () => void;
	onCopy: () => void;
	onRemove: () => void;
}) {
	return (
		<fieldset className="group-mapping-actions">
			<legend>Mapping ownership</legend>
			{presentation === "local" ? (
				<Button disabled={disabled} onClick={onRemove}>
					{hasInheritedSource
						? "Use inherited mapping"
						: "Remove local mapping"}
				</Button>
			) : (
				<Button disabled={disabled} onClick={onCreate}>
					Create local mapping
				</Button>
			)}
			{presentation === "inherited" && (
				<Button disabled={disabled} onClick={onCopy}>
					Copy inherited values as local
				</Button>
			)}
		</fieldset>
	);
}

function ProjectionEditor({
	mapping,
	disabled,
	onChange,
}: {
	mapping: SpatialSelectionMapping;
	disabled: boolean;
	onChange: (mapping: SpatialSelectionMapping) => void;
}) {
	// The same editor the Dynamics projection tab uses, so a Group and the Dynamics that
	// inherit from it are configured the same way.
	const projection = mapping.projection;
	const kind = projectionKind(projection);
	const updateProjection = (next: SpatialSelectionMapping["projection"]) =>
		onChange({ ...mapping, projection: next });
	return (
		<fieldset disabled={disabled} className="group-mapping-fields">
			<legend>Projection</legend>
			<div className="group-projection-columns">
			<div className="group-projection-visual">
				<ProjectionStagePreview projection={projection} />
				<small>Drag to orbit</small>
			</div>
			<div className="group-projection-settings">
			<SelectField
				label="Projection"
				value={kind}
				options={PROJECTION_KINDS.map(
					({ value, label }) => [value, label] as [string, string],
				)}
				onChange={(next) =>
					updateProjection(withProjectionKind(projection, next as ProjectionKind))
				}
			/>
			{supportsPreset(projection) && (
				<fieldset className="group-projection-presets">
					<legend>Projection preset</legend>
					{PROJECTION_PRESETS.map(({ value, label }) => (
						<Button
							key={value}
							aria-pressed={projection.preset === value}
							onClick={() =>
								updateProjection({
									...projectionForPreset(value),
									anchor: projection.anchor,
								})
							}
						>
							{label}
						</Button>
					))}
				</fieldset>
			)}
			<div className="group-vector-fields">
				{projectionFields(projection).map((field) => (
					<NumberField
						key={field.key}
						label={field.label}
						unit={field.unit}
						value={field.value}
						onCommit={(value) => updateProjection(field.apply(value))}
					/>
				))}
			</div>
			<p className="group-mapping-help">
				{PROJECTION_KINDS.find((entry) => entry.value === kind)?.detail}
			</p>
			</div>
			</div>
		</fieldset>
	);
}

function PhaseEditor({
	mapping,
	disabled,
	onChange,
}: {
	mapping: SpatialSelectionMapping;
	disabled: boolean;
	onChange: (mapping: SpatialSelectionMapping) => void;
}) {
	const updateShape = (shape: SpatialSelectionShape) =>
		onChange({ ...mapping, shape });
	const shape = mapping.shape;
	return (
		<fieldset disabled={disabled} className="group-mapping-fields">
			<legend>Phase ordering</legend>
			<MultiValueToggleField
				className="group-shape-choices"
				label="Ordering mode"
				value={shape.type}
				options={[
					{ value: "grid", label: "Grid" },
					{ value: "radial", label: "Radial" },
					{ value: "radar", label: "Radar" },
				]}
				onChange={(type) =>
					updateShape(defaultShape(type as SpatialSelectionShape["type"]))
				}
			/>
			<p className="group-mapping-help">
				Orders the projected plane for every Dynamic inheriting from this Group.
				Linear and Random order a Dynamic's own targets, so they are set per
				Dynamic rather than here.
			</p>
			<FormLayout labelPlacement="top">
				{shape.type === "grid" && (
					<>
						<NumberField
							label="Grid angle"
							unit="°"
							value={shape.angle_degrees}
							onCommit={(angle_degrees) =>
								updateShape({ ...shape, angle_degrees })
							}
						/>
						<SelectField
							label="Rank direction"
							value={shape.direction}
							options={[
								["ascending", "Ascending"],
								["descending", "Descending"],
							]}
							onChange={(direction) =>
								updateShape({
									...shape,
									direction: direction as "ascending" | "descending",
								})
							}
						/>
					</>
				)}
				{shape.type !== "grid" && (
					<>
						<NumberField
							label="Centre U"
							value={shape.center_u}
							onCommit={(center_u) => updateShape({ ...shape, center_u })}
						/>
						<NumberField
							label="Centre V"
							value={shape.center_v}
							onCommit={(center_v) => updateShape({ ...shape, center_v })}
						/>
					</>
				)}
				{shape.type === "radial" && (
					<SelectField
						label="Radial direction"
						value={shape.direction}
						options={[
							["outward", "Outward"],
							["inward", "Inward"],
						]}
						onChange={(direction) =>
							updateShape({
								...shape,
								direction: direction as "outward" | "inward",
							})
						}
					/>
				)}
				{shape.type === "radar" && (
					<>
						<NumberField
							label="Start angle"
							unit="°"
							value={shape.start_angle_degrees}
							onCommit={(start_angle_degrees) =>
								updateShape({ ...shape, start_angle_degrees })
							}
						/>
						<SelectField
							label="Sweep"
							value={shape.sweep}
							options={[
								["clockwise", "Clockwise"],
								["counter_clockwise", "Counter-clockwise"],
							]}
							onChange={(sweep) =>
								updateShape({
									...shape,
									sweep: sweep as "clockwise" | "counter_clockwise",
								})
							}
						/>
					</>
				)}
			</FormLayout>
		</fieldset>
	);
}

function NumberField({
	label,
	value,
	unit,
	onCommit,
}: {
	label: string;
	value: number;
	unit?: string;
	onCommit: (value: number) => void;
}) {
	return (
		<UiNumberField
			className="group-number-field"
			label={label}
			unit={unit}
			defaultValue={value}
			allowDecimal
			showStepButtons={false}
			onBlur={(event) => onCommit(Number(event.currentTarget.value))}
			onKeyDown={(event) => {
				if (event.key === "Enter") event.currentTarget.blur();
			}}
		/>
	);
}

function SelectField({
	label,
	value,
	options,
	onChange,
}: {
	label: string;
	value: string;
	options: Array<[string, string]>;
	onChange: (value: string) => void;
}) {
	return (
		<UiSelectField
			className="group-number-field"
			label={label}
			ariaLabel={label}
			value={value}
			options={options.map(([option, title]) => ({
				value: option,
				label: title,
			}))}
			onChange={onChange}
		/>
	);
}

function MappingPreview({
	title,
	fixtures,
	resolved,
}: {
	title: string;
	fixtures: number;
	resolved: ResolvedSpatialMapping | null;
}) {
	return (
		<section className="group-mapping-preview" aria-label={title}>
			<strong>{title}</strong>
			{resolved ? (
				<>
					<p>
						{resolved.ordered_fixture_ids.length} source fixtures ·{" "}
						{resolved.rank_count} authoritative ranks
					</p>
					<ol className="group-mapping-ranks">
						{resolved.projected_positions.slice(0, 24).map((position) => {
							const rank = resolved.ranks.find(
								(candidate) => candidate.fixture_id === position.fixture_id,
							);
							return (
								<li key={position.fixture_id}>
									<code>{position.fixture_id}</code>
									<span>
										{position.u == null || position.v == null
											? "U — · V —"
											: `U ${formatCoordinate(position.u)} · V ${formatCoordinate(position.v)}`}
									</span>
									<span>{rank ? `Rank ${rank.rank + 1}` : "No rank"}</span>
								</li>
							);
						})}
					</ol>
					{resolved.warnings.map((warning) => (
						<p className="group-mapping-warning" key={warning.fixture_id}>
							Fixture {warning.fixture_id} has no Stage position and uses its
							own fallback rank.
						</p>
					))}
				</>
			) : (
				<p>
					{fixtures
						? "Loading authoritative Stage ranks…"
						: "This intentionally empty Group has no ranked positions."}
				</p>
			)}
		</section>
	);
}

function formatCoordinate(value: number) {
	return value.toFixed(3).replace(/0+$/u, "").replace(/\.$/u, "");
}

function resolvedMappingPresentation(
	resolved: ResolvedSpatialMapping,
): ReturnType<typeof resolveMappingPresentation> {
	const mapping = resolved.effective_mapping ?? null;
	switch (resolved.mapping_provenance.type) {
		case "local":
			return mapping
				? { type: "local", label: "Local override", mapping }
				: { type: "none", label: "Mapping: None", mapping: null };
		case "inherited":
			return mapping
				? {
						type: "inherited",
						label: `Inherited from ${resolved.mapping_provenance.source_group_ids
							.map((id) => `Group ${id}`)
							.join(" · ")}`,
						mapping,
						sourceGroupIds: resolved.mapping_provenance.source_group_ids,
					}
				: { type: "none", label: "Mapping: None", mapping: null };
		case "mixed_source_mappings":
			return {
				type: "mixed",
				label: "Mixed source mappings — source order",
				mapping: null,
			};
		case "none":
			return { type: "none", label: "Mapping: None", mapping: null };
	}
}

function SaveStatus({
	status,
	saving,
}: {
	status: string | null;
	saving: boolean;
}) {
	if (!status && !saving) return null;
	return (
		<p className="group-settings-status" role="status">
			{saving ? "Saving…" : status}
		</p>
	);
}

function defaultShape(
	type: SpatialSelectionShape["type"],
): SpatialSelectionShape {
	if (type === "radial")
		return { type, center_u: 0, center_v: 0, direction: "outward" };
	if (type === "radar")
		return {
			type,
			center_u: 0,
			center_v: 0,
			start_angle_degrees: 0,
			sweep: "clockwise",
		};
	return { type, angle_degrees: 0, direction: "ascending" };
}

function titleCase(value: string) {
	return `${value.charAt(0).toUpperCase()}${value.slice(1)}`;
}
