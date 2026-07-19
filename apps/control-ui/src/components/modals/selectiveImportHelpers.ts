import type {
	SelectiveImportCatalog,
	SelectiveImportConflictResolution,
	SelectiveImportObjectKey,
	SelectiveImportProfileConflictResolution,
	SelectiveImportProfileKey,
	SelectiveImportSelection,
} from "../../api/selectiveImportModels";

const ACTION_LABELS: Record<string, string> = {
	blocked_conflict: "Needs Resolution",
	copy: "Copy",
	duplicate: "Import as Copy",
	import_preserving_id: "Import",
	keep_destination: "Keep Destination",
	missing: "Missing",
	replace_destination: "Replace Destination",
	skip_identical: "Skip Identical",
};

export function buildSelection(
	catalog: SelectiveImportCatalog | null,
	selected: Set<string>,
	objectChoices: Map<string, SelectiveImportConflictResolution>,
	profileChoices: Map<string, SelectiveImportProfileConflictResolution>,
): SelectiveImportSelection {
	return {
		selectedObjects: catalog?.objects
			.filter((object) => selected.has(objectKeyId(object.key)))
			.map((object) => object.key) ?? [],
		conflictResolutions: [...objectChoices].map(([encoded, resolution]) => ({
			key: decodeObjectKey(encoded),
			resolution,
		})),
		profileConflictResolutions: [...profileChoices].map(([encoded, resolution]) => ({
			key: decodeProfileKey(encoded),
			resolution,
		})),
	};
}

export function updatedMap<T>(source: Map<string, T>, key: string, value: T | null) {
	const next = new Map(source);
	if (value === null) next.delete(key);
	else next.set(key, value);
	return next;
}

export function toggledSet(source: Set<string>, key: string, checked: boolean) {
	const next = new Set(source);
	if (checked) next.add(key);
	else next.delete(key);
	return next;
}

export function objectKeyId(key: SelectiveImportObjectKey) {
	return JSON.stringify([key.kind, key.id]);
}

export function profileKeyId(key: SelectiveImportProfileKey) {
	return `${key.profileId}@${key.revision}`;
}

export function humanize(value: string) {
	return value
		.replaceAll("_", " ")
		.replace(/\b\w/g, (letter) => letter.toUpperCase());
}

export function actionLabel(value: string) {
	return ACTION_LABELS[value] ?? humanize(value);
}

export function operatorError(reason: unknown) {
	const message = reason instanceof Error ? reason.message : String(reason);
	try {
		const body = JSON.parse(message) as { error?: string };
		return body.error ?? message;
	} catch {
		return message;
	}
}

function decodeObjectKey(value: string): SelectiveImportObjectKey {
	const [kind, id] = JSON.parse(value) as [string, string];
	return { kind, id };
}

function decodeProfileKey(value: string): SelectiveImportProfileKey {
	const split = value.lastIndexOf("@");
	return {
		profileId: value.slice(0, split),
		revision: Number(value.slice(split + 1)),
	};
}
