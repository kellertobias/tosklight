import type {
	SelectiveImportCatalog,
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
				<label key={objectKeyId(object.key)}>
					<input
						type="checkbox"
						disabled={disabled}
						checked={selected.has(objectKeyId(object.key))}
						onChange={(event) => onChange(object.key, event.target.checked)}
					/>
					{object.displayName}{" "}
					<small>{humanize(object.key.kind)} · {object.key.id}</small>
				</label>
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
			<label key={objectKeyId(conflict.key)}>
				{conflict.key.kind}/{conflict.key.id}
				<select
					aria-label={`Resolve ${conflict.key.kind} ${conflict.key.id}`}
					disabled={props.disabled}
					value={props.objectChoices.get(objectKeyId(conflict.key)) ?? conflict.resolution ?? ""}
					onChange={(event) => props.onObjectChoice(
						conflict.key,
						event.target.value
							? event.target.value as SelectiveImportConflictResolution
							: null,
					)}
				>
					<option value="">Choose resolution…</option>
					<option value="keep_destination">Keep Destination</option>
					<option value="replace_destination">Replace Destination</option>
					<option value="duplicate">Import as Copy</option>
				</select>
			</label>
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
					<select
						aria-label={`Resolve profile ${profile.source.profileId} revision ${profile.source.revision}`}
						disabled={props.disabled}
						value={props.profileChoices.get(profileKeyId(profile.source)) ?? ""}
						onChange={(event) => props.onProfileChoice(
							profile.source,
							event.target.value
								? event.target.value as SelectiveImportProfileConflictResolution
								: null,
						)}
					>
						<option value="">Choose resolution…</option>
						<option value="keep_destination">Keep Destination</option>
						<option value="duplicate">Import as Copy</option>
					</select>
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
