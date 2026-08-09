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
import { PhaseOrderPreview } from "../../features/spatialMapping/PhaseOrderPreview";
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
		setStatus(outcome.persistenceWarning ?? null);
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
		// Reflect the accepted operator gesture while the revisioned write is in flight. A
		// rejection repairs from authority below; waiting for the round trip made projection-kind
		// changes appear not to work, especially when the selected kind changed the whole form.
		setMapping(next);
		setMappingPresentation(
			next
				? { type: "local", label: "Local override", mapping: next }
				: resolveMappingPresentation(
						{ ...group, body: { ...group.body, mapping: undefined } } as Group,
						groups,
					),
		);
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
		await refreshSettings();
		setStatus(null);
	};

	const editingUnavailable = !groupManagement
		? "Spatial mapping editing is unavailable until the revisioned Group mapping action is connected."
		: null;
	const displayedMapping = mapping ?? mappingPresentation.mapping;
	const commonPanelProps = {
		mappingPresentation,
		displayedMapping,
		resolvedSpatial,
		saving,
		hasManagement: Boolean(groupManagement),
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
	mappingPresentation: ReturnType<typeof resolveMappingPresentation>;
	displayedMapping: SpatialSelectionMapping | null;
	resolvedSpatial: ResolvedSpatialMapping | null;
	saving: boolean;
	hasManagement: boolean;
	commitMapping(next: SpatialSelectionMapping | null): Promise<void>;
}

/**
 * Two sections side by side: the preview on the left, everything that is set on the right.
 *
 * Mapping ownership is not offered here. Opening a Group's Projection is asking to set this
 * Group's projection, so the first edit takes ownership on its own, and the operator is never
 * asked to press a button before the numbers do anything.
 */
function ProjectionSettingsPanel(
	props: MappingPanelProps & { editingUnavailable: string | null },
) {
	const {
		displayedMapping,
		saving,
		commitMapping,
		editingUnavailable,
		hasManagement,
	} = props;
	// An inherited mapping is edited by making it this Group's own, which is what changing it
	// means. Until there is one to copy, the first edit starts from the default.
	const editable = displayedMapping ?? defaultSpatialMapping();
	return (
		<section className="group-settings-panel group-settings-split">
			<div className="group-settings-visual" aria-label="Projection preview">
				<h3>Projection preview</h3>
				<ProjectionStagePreview projection={editable.projection} />
				<small>Drag to orbit</small>
			</div>
			<div className="group-settings-controls">
				{editingUnavailable && <p role="note">{editingUnavailable}</p>}
				<ProjectionEditor
					mapping={editable}
					disabled={!hasManagement || saving}
					onChange={(next) => void commitMapping(next)}
				/>
			</div>
		</section>
	);
}

function PhaseSettingsPanel({
	displayedMapping,
	resolvedSpatial,
	saving,
	hasManagement,
	commitMapping,
}: MappingPanelProps) {
	const editable = displayedMapping ?? defaultSpatialMapping();
	return (
		<section className="group-settings-panel group-settings-split">
			<div className="group-settings-visual" aria-label="Phase preview">
				<h3>Phase preview</h3>
				{resolvedSpatial ? (
					<PhaseOrderPreview
						positions={resolvedSpatial.projected_positions}
						ranks={resolvedSpatial.ranks}
						rankCount={resolvedSpatial.rank_count}
						unplaced={resolvedSpatial.warnings.length}
					/>
				) : (
					<p className="phase-order-empty">
						Loading authoritative Stage ranks…
					</p>
				)}
			</div>
			<div className="group-settings-controls">
				<PhaseEditor
					mapping={editable}
					disabled={!hasManagement || saving}
					onChange={(next) => void commitMapping(next)}
				/>
			</div>
		</section>
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
	const fields = projectionFields(projection);
	const positionFields = fields.filter((field) =>
		field.key.startsWith("position-"),
	);
	const directionFields = fields.filter(
		(field) => !field.key.startsWith("position-"),
	);
	const updateProjection = (next: SpatialSelectionMapping["projection"]) =>
		onChange({ ...mapping, projection: next });
	return (
		<fieldset
			disabled={disabled}
			className="group-mapping-fields group-projection-fields"
		>
			<legend>Projection</legend>
			<MultiValueToggleField
				className="group-projection-kinds"
				label="Projection"
				value={kind}
				options={PROJECTION_KINDS.map(({ value, label }) => ({ value, label }))}
				onChange={(next) =>
					updateProjection(
						withProjectionKind(projection, next as ProjectionKind),
					)
				}
			/>
			<div className="group-vector-fields">
				{supportsPreset(projection) ? (
					<fieldset className="group-vector-column group-preset-fields">
						<legend>Presets</legend>
						<div className="group-projection-presets">
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
						</div>
					</fieldset>
				) : positionFields.length > 0 ? (
					<fieldset className="group-vector-column group-position-fields">
						<legend>Position</legend>
						{positionFields.map((field) => (
							<NumberField
								key={`${kind}-${field.key}`}
								label={field.label}
								unit={field.unit}
								value={field.value}
								showStepButtons={false}
								onCommit={(value) => updateProjection(field.apply(value))}
							/>
						))}
					</fieldset>
				) : null}
				<fieldset className="group-vector-column group-direction-fields">
					<legend>Direction</legend>
					{directionFields.map((field) => (
						<NumberField
							key={`${kind}-${field.key}`}
							label={field.label}
							unit={field.unit}
							value={field.value}
							angle={field.unit === "°"}
							onCommit={(value) => updateProjection(field.apply(value))}
						/>
					))}
				</fieldset>
			</div>
			<p className="group-mapping-help">
				{PROJECTION_KINDS.find((entry) => entry.value === kind)?.detail}
			</p>
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
	const [orderingMode, setOrderingMode] = useState<
		"fixture-id" | "selection-order" | SpatialSelectionShape["type"]
	>("selection-order");
	const spatialMode =
		orderingMode === "grid" ||
		orderingMode === "radial" ||
		orderingMode === "radar";
	// A revisioned write may still be fetching authority when an operator changes modes. Render
	// the chosen shape immediately rather than briefly showing the previous shape's controls.
	const activeShape =
		spatialMode && shape.type !== orderingMode
			? defaultShape(orderingMode)
			: shape;
	return (
		<fieldset
			disabled={disabled}
			className="group-mapping-fields group-phase-fields"
		>
			<legend>Phase ordering</legend>
			<MultiValueToggleField
				className="group-shape-choices"
				label="Ordering mode"
				value={orderingMode}
				options={[
					{ value: "selection-order", label: "Selection Order" },
					{ value: "fixture-id", label: "Fixture ID" },
					{ value: "grid", label: "Grid" },
					{ value: "radial", label: "Radial" },
					{ value: "radar", label: "Radar" },
				]}
				onChange={(type) => {
					const next = type as typeof orderingMode;
					setOrderingMode(next);
					if (next === "grid" || next === "radial" || next === "radar")
						updateShape(defaultShape(next));
				}}
			/>
			{!spatialMode ? (
				<div className="group-phase-no-settings">
					<strong>
						{orderingMode === "selection-order"
							? "Selection Order"
							: "Fixture ID"}
					</strong>
					<p>
						{orderingMode === "selection-order"
							? "Uses the order in which the fixtures were selected when the Group was stored."
							: "Orders fixtures by Fixture ID, from the lowest ID to the highest."}
					</p>
				</div>
			) : (
				<>
					<p className="group-mapping-help">
						Orders the projected plane for every Dynamic inheriting from this
						Group. Linear and Random order a Dynamic's own targets, so they are
						set per Dynamic rather than here.
					</p>
					<FormLayout labelPlacement="top">
						{activeShape.type === "grid" && (
							<div className="group-phase-row">
								<NumberField
									label="Grid angle"
									labelPlacement="top"
									unit="°"
									value={activeShape.angle_degrees}
									onCommit={(angle_degrees) =>
										updateShape({ ...activeShape, angle_degrees })
									}
								/>
								<SelectField
									label="Rank direction"
									labelPlacement="top"
									value={activeShape.direction}
									options={[
										["ascending", "Ascending"],
										["descending", "Descending"],
									]}
									onChange={(direction) =>
										updateShape({
											...activeShape,
											direction: direction as "ascending" | "descending",
										})
									}
								/>
							</div>
						)}
						{activeShape.type !== "grid" && (
							<div className="group-phase-row">
								<NumberField
									label="Centre U"
									labelPlacement="top"
									value={activeShape.center_u}
									onCommit={(center_u) =>
										updateShape({ ...activeShape, center_u })
									}
								/>
								<NumberField
									label="Centre V"
									labelPlacement="top"
									value={activeShape.center_v}
									onCommit={(center_v) =>
										updateShape({ ...activeShape, center_v })
									}
								/>
							</div>
						)}
						{activeShape.type === "radial" && (
							<div className="group-phase-row group-phase-single">
								<SelectField
									label="Radial direction"
									labelPlacement="top"
									value={activeShape.direction}
									options={[
										["outward", "Outward"],
										["inward", "Inward"],
									]}
									onChange={(direction) =>
										updateShape({
											...activeShape,
											direction: direction as "outward" | "inward",
										})
									}
								/>
							</div>
						)}
						{activeShape.type === "radar" && (
							<div className="group-phase-row">
								<NumberField
									label="Start angle"
									labelPlacement="top"
									unit="°"
									value={activeShape.start_angle_degrees}
									onCommit={(start_angle_degrees) =>
										updateShape({ ...activeShape, start_angle_degrees })
									}
								/>
								<SelectField
									label="Sweep"
									labelPlacement="top"
									value={activeShape.sweep}
									options={[
										["clockwise", "Clockwise"],
										["counter_clockwise", "Counter-clockwise"],
									]}
									onChange={(sweep) =>
										updateShape({
											...activeShape,
											sweep: sweep as "clockwise" | "counter_clockwise",
										})
									}
								/>
							</div>
						)}
					</FormLayout>
				</>
			)}
		</fieldset>
	);
}

function NumberField({
	label,
	value,
	unit,
	angle = false,
	showStepButtons = angle,
	labelPlacement = "side",
	onCommit,
}: {
	label: string;
	value: number;
	unit?: string;
	angle?: boolean;
	showStepButtons?: boolean;
	labelPlacement?: "side" | "top";
	onCommit: (value: number) => void;
}) {
	return (
		<UiNumberField
			className="group-number-input"
			label={label}
			labelPlacement={labelPlacement}
			unit={unit}
			defaultValue={value}
			allowDecimal
			showStepButtons={showStepButtons}
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

function SelectField({
	label,
	value,
	options,
	labelPlacement = "side",
	onChange,
}: {
	label: string;
	value: string;
	options: Array<[string, string]>;
	labelPlacement?: "side" | "top";
	onChange: (value: string) => void;
}) {
	return (
		<UiSelectField
			className="group-select-field"
			label={label}
			labelPlacement={labelPlacement}
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
	return (
		<p className="group-settings-status" role="status">
			{saving ? "Saving…" : (status ?? "")}
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
