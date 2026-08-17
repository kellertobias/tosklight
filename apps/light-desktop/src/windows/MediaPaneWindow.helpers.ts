import type { MediaServerInspection } from "../api/client/mediaOutput";

export function mediaDraftForLayer(
	inspection: MediaServerInspection,
	layers: ReadonlyArray<{ fixture_id: string; head_index: number }>,
	layerId: string,
): { folderId: string; fileId: string } | null {
	const head = layers.find((candidate) => candidate.fixture_id === layerId);
	if (!head) return null;
	const citpLayer = layers.findIndex(
		(candidate) => candidate.fixture_id === layerId,
	);
	const status = inspection.layers.find(
		(candidate) => candidate.layer === citpLayer,
	);
	return status
		? {
				folderId: String(status.folder === 0 ? 1 : status.folder),
				fileId: String(status.file),
			}
		: null;
}

export function mediaCapabilitiesForLayer(
	inspection: MediaServerInspection,
	headIndex: number | undefined,
) {
	return headIndex == null
		? undefined
		: inspection.capabilities.layers.find(
				(capability) => capability.layer === headIndex,
			);
}

export function mediaFileMutations(
	fixtureId: string,
	folder: number,
	file: number,
) {
	return mediaLibraryMutations(fixtureId, "media", folder, file);
}

export function mediaLibraryMutations(
	fixtureId: string,
	mode: "media" | "mask",
	folder: number,
	file: number,
) {
	const prefix = mode === "mask" ? "media.mask" : "media";
	return [
		mediaMutation(fixtureId, `${prefix}.folder`, folder),
		mediaMutation(fixtureId, `${prefix}.file`, file),
	];
}

function mediaMutation(fixtureId: string, attribute: string, value: number) {
	return {
		action: "set_fixture" as const,
		fixtureId,
		attribute,
		value: {
			kind: "normalized" as const,
			value: Math.max(0, Math.min(255, value)) / 255,
		},
		timing: { fade: false, fadeMillis: null, delayMillis: null },
	};
}
