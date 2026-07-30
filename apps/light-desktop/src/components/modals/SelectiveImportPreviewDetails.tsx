import { CheckboxField, SelectField } from "@tosklight/ui";
import type {
	SelectiveImportCatalog,
	SelectiveImportCatalogSection,
	SelectiveImportConflictResolution,
	SelectiveImportObjectKey,
	SelectiveImportPreview,
	SelectiveImportProfileConflictResolution,
	SelectiveImportProfileKey,
} from "../../api/selectiveImportModels";
import {
	actionLabel,
	humanize,
	objectKeyId,
	profileKeyId,
} from "./selectiveImportHelpers";

export function CatalogSelection({
	catalog,
	selected,
	disabled,
	onChange,
}: {
	catalog: SelectiveImportCatalog;
	selected: Set<string>;
	disabled: boolean;
	onChange: (key: SelectiveImportObjectKey, checked: boolean) => void;
}) {
	const sections = catalog.objects.reduce((grouped, object) => {
		const objects = grouped.get(object.section) ?? [];
		objects.push(object);
		grouped.set(object.section, objects);
		return grouped;
	}, new Map<
		SelectiveImportCatalogSection,
		SelectiveImportCatalog["objects"]
	>());
	return (
		<div>
			<h4>Source Content ({catalog.objects.length})</h4>
			{catalog.objects.length === 0 && (
				<p>No portable show objects are available.</p>
			)}
			{[...sections].map(([section, objects]) => {
				const selectedCount = objects.filter((object) =>
					selected.has(objectKeyId(object.key)),
				).length;
				return (
					<section key={section} aria-label={sectionLabel(section)}>
						<CheckboxField
							label={`${sectionLabel(section)} (${selectedCount}/${objects.length})`}
							stateLabel="Select section"
							aria-label={`Select all ${sectionLabel(section)}`}
							disabled={disabled}
							checked={selectedCount === objects.length}
							onChange={(event) => {
								for (const object of objects)
									onChange(object.key, event.target.checked);
							}}
						/>
						{section === "fixture_patch" &&
							[
								...new Set(
									objects.flatMap((object) =>
										object.patchLayerId ? [object.patchLayerId] : [],
									),
								),
							].map((layerId) => {
								const layerObjects = objects.filter(
									(object) => object.patchLayerId === layerId,
								);
								const layerSelected = layerObjects.filter((object) =>
									selected.has(objectKeyId(object.key)),
								).length;
								return (
									<CheckboxField
										key={`patch-layer:${layerId}`}
										label={`Patch layer ${layerId} (${layerSelected}/${layerObjects.length})`}
										stateLabel="Select patch layer"
										aria-label={`Select patch layer ${layerId}`}
										disabled={disabled}
										checked={layerSelected === layerObjects.length}
										onChange={(event) => {
											for (const object of layerObjects) {
												onChange(object.key, event.target.checked);
											}
										}}
									/>
								);
							})}
						{objects.map((object) => (
							<CheckboxField
								key={objectKeyId(object.key)}
								label={
									<>
										{object.displayName}{" "}
										<small>
											{humanize(object.key.kind)} · {object.key.id}
											{object.patchLayerId
												? ` · Layer ${object.patchLayerId}`
												: ""}
										</small>
									</>
								}
								stateLabel="Include in import"
								aria-label={object.displayName}
								disabled={disabled}
								checked={selected.has(objectKeyId(object.key))}
								onChange={(event) => onChange(object.key, event.target.checked)}
							/>
						))}
					</section>
				);
			})}
		</div>
	);
}

const SECTION_LABELS: Record<SelectiveImportCatalogSection, string> = {
	fixture_patch: "Fixture Patch",
	groups: "Groups",
	presets_mixed: "Mixed Presets",
	presets_intensity: "Intensity Presets",
	presets_color: "Color Presets",
	presets_position: "Position Presets",
	presets_beam: "Beam Presets",
	dynamics: "Dynamics",
	cuelists: "Cuelists and Cues",
	playbacks: "Playbacks and Page Assignments",
	schedules: "Schedules",
	stage: "Stage",
	macros: "Macros",
	other: "Other Show Content",
};

function sectionLabel(section: SelectiveImportCatalogSection) {
	return SECTION_LABELS[section];
}

export function PreviewDetails(props: {
	preview: SelectiveImportPreview;
	disabled: boolean;
	objectChoices: Map<string, SelectiveImportConflictResolution>;
	profileChoices: Map<string, SelectiveImportProfileConflictResolution>;
	onObjectChoice: (
		key: SelectiveImportObjectKey,
		value: SelectiveImportConflictResolution | null,
	) => void;
	onProfileChoice: (
		key: SelectiveImportProfileKey,
		value: SelectiveImportProfileConflictResolution | null,
	) => void;
}) {
	return (
		<section aria-label="Selective Show Import preview">
			<ObjectSummary preview={props.preview} />
			<DependencySummary preview={props.preview} />
			<ConflictChoices {...props} />
			<ProfileChoices {...props} />
			<ManagedAssetSummary preview={props.preview} />
			<BlockerSummary preview={props.preview} />
		</section>
	);
}

function ObjectSummary({ preview }: { preview: SelectiveImportPreview }) {
	return (
		<>
			<h4>Objects ({preview.objects.length})</h4>
			{preview.objects.map((object) => (
				<p key={objectKeyId(object.source)}>
					{humanize(object.source.kind)} {object.source.id}:{" "}
					{actionLabel(object.action)}
				</p>
			))}
		</>
	);
}

function DependencySummary({ preview }: { preview: SelectiveImportPreview }) {
	return (
		<>
			<h4>Dependencies ({preview.dependencies.length})</h4>
			{preview.dependencies.length === 0 ? (
				<p>None</p>
			) : (
				preview.dependencies.map((item) => (
					<p key={`${objectKeyId(item.owner)}>${objectKeyId(item.dependency)}`}>
						{item.owner.id} → {item.dependency.kind}/{item.dependency.id}:{" "}
						{humanize(item.disposition)}
					</p>
				))
			)}
		</>
	);
}

function ConflictChoices(
	props: Pick<
		Parameters<typeof PreviewDetails>[0],
		"preview" | "disabled" | "objectChoices" | "onObjectChoice"
	>,
) {
	return (
		<>
			<h4>Conflicts ({props.preview.conflicts.length})</h4>
			{props.preview.conflicts.length === 0 ? (
				<p>None</p>
			) : (
				props.preview.conflicts.map((conflict) => (
					<SelectField
						key={objectKeyId(conflict.key)}
						label={`Resolve ${conflict.key.kind} ${conflict.key.id}`}
						ariaLabel={`Resolve ${conflict.key.kind} ${conflict.key.id}`}
						disabled={props.disabled}
						value={
							props.objectChoices.get(objectKeyId(conflict.key)) ??
							conflict.resolution ??
							""
						}
						onChange={(value) =>
							props.onObjectChoice(
								conflict.key,
								value ? (value as SelectiveImportConflictResolution) : null,
							)
						}
						options={[
							{ value: "", label: "Choose resolution…" },
							{ value: "keep_destination", label: "Keep Destination" },
							{ value: "replace_destination", label: "Replace Destination" },
							{ value: "duplicate", label: "Import as Copy" },
						]}
					/>
				))
			)}
		</>
	);
}

function ProfileChoices(
	props: Pick<
		Parameters<typeof PreviewDetails>[0],
		"preview" | "disabled" | "profileChoices" | "onProfileChoice"
	>,
) {
	return (
		<>
			<h4>Fixture Profiles ({props.preview.profiles.length})</h4>
			{props.preview.profiles.length === 0 ? (
				<p>None</p>
			) : (
				props.preview.profiles.map((profile) => (
					<div key={profileKeyId(profile.source)}>
						<p>
							{profile.source.profileId} Revision {profile.source.revision}:{" "}
							{actionLabel(profile.action)}
						</p>
						{profile.action === "blocked_conflict" && (
							<SelectField
								label={`Resolve profile ${profile.source.profileId} revision ${profile.source.revision}`}
								ariaLabel={`Resolve profile ${profile.source.profileId} revision ${profile.source.revision}`}
								disabled={props.disabled}
								value={
									props.profileChoices.get(profileKeyId(profile.source)) ?? ""
								}
								onChange={(value) =>
									props.onProfileChoice(
										profile.source,
										value
											? (value as SelectiveImportProfileConflictResolution)
											: null,
									)
								}
								options={[
									{ value: "", label: "Choose resolution…" },
									{ value: "keep_destination", label: "Keep Destination" },
									{ value: "duplicate", label: "Import as Copy" },
								]}
							/>
						)}
					</div>
				))
			)}
		</>
	);
}

function ManagedAssetSummary({ preview }: { preview: SelectiveImportPreview }) {
	return (
		<>
			<h4>Managed Assets ({preview.managedAssets.length})</h4>
			{preview.managedAssets.length === 0 ? (
				<p>None</p>
			) : (
				preview.managedAssets.map(({ asset, action }) => (
					<p key={`${asset.assetId}@${asset.revision}`}>
						{asset.assetId} Revision {asset.revision}: {humanize(action)}
					</p>
				))
			)}
		</>
	);
}

function BlockerSummary({ preview }: { preview: SelectiveImportPreview }) {
	return (
		<>
			<h4>Blocking Problems ({preview.blockers.length})</h4>
			{preview.blockers.length === 0 ? (
				<p>None — ready to apply.</p>
			) : (
				preview.blockers.map((blocker, index) => (
					<p className="modal-error" key={`${blocker.type}-${index}`}>
						{blocker.summary}
					</p>
				))
			)}
		</>
	);
}
