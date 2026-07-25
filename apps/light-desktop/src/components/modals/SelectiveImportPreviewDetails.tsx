import type {
	SelectiveImportCatalog,
	SelectiveImportConflictResolution,
	SelectiveImportObjectKey,
	SelectiveImportPreview,
	SelectiveImportProfileConflictResolution,
	SelectiveImportProfileKey,
} from "../../api/selectiveImportModels";
import { CheckboxField, SelectField } from "../common";
import {
	actionLabel,
	humanize,
	objectKeyId,
	profileKeyId,
} from "./selectiveImportHelpers";

export function CatalogSelection({ catalog, selected, disabled, onChange }: {
	catalog: SelectiveImportCatalog;
	selected: Set<string>;
	disabled: boolean;
	onChange: (key: SelectiveImportObjectKey, checked: boolean) => void;
}) {
	return (
		<div>
			<h4>Source Content ({catalog.objects.length})</h4>
			{catalog.objects.length === 0 && <p>No portable show objects are available.</p>}
			{catalog.objects.map((object) => (
				<CheckboxField
					key={objectKeyId(object.key)}
					label={<>{object.displayName}{" "}
						<small>{humanize(object.key.kind)} · {object.key.id}</small></>}
					aria-label={object.displayName}
						disabled={disabled}
						checked={selected.has(objectKeyId(object.key))}
						onChange={(event) => onChange(object.key, event.target.checked)}
				/>
			))}
		</div>
	);
}

export function PreviewDetails(props: {
	preview: SelectiveImportPreview;
	disabled: boolean;
	objectChoices: Map<string, SelectiveImportConflictResolution>;
	profileChoices: Map<string, SelectiveImportProfileConflictResolution>;
	onObjectChoice: (key: SelectiveImportObjectKey, value: SelectiveImportConflictResolution | null) => void;
	onProfileChoice: (key: SelectiveImportProfileKey, value: SelectiveImportProfileConflictResolution | null) => void;
}) {
	return (
		<div aria-label="Selective Show Import preview">
			<ObjectSummary preview={props.preview} />
			<DependencySummary preview={props.preview} />
			<ConflictChoices {...props} />
			<ProfileChoices {...props} />
			<ManagedAssetSummary preview={props.preview} />
			<BlockerSummary preview={props.preview} />
		</div>
	);
}

function ObjectSummary({ preview }: { preview: SelectiveImportPreview }) {
	return <>
		<h4>Objects ({preview.objects.length})</h4>
		{preview.objects.map((object) => (
			<p key={objectKeyId(object.source)}>
				{humanize(object.source.kind)} {object.source.id}: {actionLabel(object.action)}
			</p>
		))}
	</>;
}

function DependencySummary({ preview }: { preview: SelectiveImportPreview }) {
	return <>
		<h4>Dependencies ({preview.dependencies.length})</h4>
		{preview.dependencies.length === 0 ? <p>None</p> : preview.dependencies.map((item) => (
			<p key={`${objectKeyId(item.owner)}>${objectKeyId(item.dependency)}`}>
				{item.owner.id} → {item.dependency.kind}/{item.dependency.id}: {humanize(item.disposition)}
			</p>
		))}
	</>;
}

function ConflictChoices(props: Pick<
	Parameters<typeof PreviewDetails>[0],
	"preview" | "disabled" | "objectChoices" | "onObjectChoice"
>) {
	return <>
		<h4>Conflicts ({props.preview.conflicts.length})</h4>
		{props.preview.conflicts.length === 0 ? <p>None</p> : props.preview.conflicts.map((conflict) => (
			<SelectField
				key={objectKeyId(conflict.key)}
				label={`Resolve ${conflict.key.kind} ${conflict.key.id}`}
				ariaLabel={`Resolve ${conflict.key.kind} ${conflict.key.id}`}
					disabled={props.disabled}
					value={props.objectChoices.get(objectKeyId(conflict.key)) ?? conflict.resolution ?? ""}
					onChange={(value) => props.onObjectChoice(
						conflict.key,
						value
							? value as SelectiveImportConflictResolution
							: null,
					)}
				options={[
					{ value: "", label: "Choose resolution…" },
					{ value: "keep_destination", label: "Keep Destination" },
					{ value: "replace_destination", label: "Replace Destination" },
					{ value: "duplicate", label: "Import as Copy" },
				]}
			/>
		))}
	</>;
}

function ProfileChoices(props: Pick<
	Parameters<typeof PreviewDetails>[0],
	"preview" | "disabled" | "profileChoices" | "onProfileChoice"
>) {
	return <>
		<h4>Fixture Profiles ({props.preview.profiles.length})</h4>
		{props.preview.profiles.length === 0 ? <p>None</p> : props.preview.profiles.map((profile) => (
			<div key={profileKeyId(profile.source)}>
				<p>{profile.source.profileId} Revision {profile.source.revision}: {actionLabel(profile.action)}</p>
				{profile.action === "blocked_conflict" && (
					<SelectField
						label={`Resolve profile ${profile.source.profileId} revision ${profile.source.revision}`}
						ariaLabel={`Resolve profile ${profile.source.profileId} revision ${profile.source.revision}`}
						disabled={props.disabled}
						value={props.profileChoices.get(profileKeyId(profile.source)) ?? ""}
						onChange={(value) => props.onProfileChoice(
							profile.source,
							value
								? value as SelectiveImportProfileConflictResolution
								: null,
						)}
						options={[
							{ value: "", label: "Choose resolution…" },
							{ value: "keep_destination", label: "Keep Destination" },
							{ value: "duplicate", label: "Import as Copy" },
						]}
					/>
				)}
			</div>
		))}
	</>;
}

function ManagedAssetSummary({ preview }: { preview: SelectiveImportPreview }) {
	return <>
		<h4>Managed Assets ({preview.managedAssets.length})</h4>
		{preview.managedAssets.length === 0 ? <p>None</p> : preview.managedAssets.map(({ asset, action }) => (
			<p key={`${asset.assetId}@${asset.revision}`}>
				{asset.assetId} Revision {asset.revision}: {humanize(action)}
			</p>
		))}
	</>;
}

function BlockerSummary({ preview }: { preview: SelectiveImportPreview }) {
	return <>
		<h4>Blocking Problems ({preview.blockers.length})</h4>
		{preview.blockers.length === 0 ? <p>None — ready to apply.</p> : preview.blockers.map((blocker, index) => (
			<p className="modal-error" key={`${blocker.type}-${index}`}>{blocker.summary}</p>
		))}
	</>;
}
