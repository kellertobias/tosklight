import type { MediaServerInspection } from "../api/client/mediaOutput";

export function mediaDraftForLayer(
	inspection: MediaServerInspection,
	layers: ReadonlyArray<{ fixture_id: string; head_index: number }>,
	layerId: string,
): { folderId: string; fileId: string } | null {
	const head = layers.find((candidate) => candidate.fixture_id === layerId);
	if (!head) return null;
	const status = inspection.layers.find(
		(candidate) => candidate.layer === head.head_index,
	);
	return status
		? { folderId: String(status.folder), fileId: String(status.file) }
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
	return [
		mediaMutation(fixtureId, "media.folder", folder),
		mediaMutation(fixtureId, "media.file", file),
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
