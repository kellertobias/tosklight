import type { PlaybackDefinition, PlaybackPage } from "../../api/types";
export function playbackSlotAssignments(page: PlaybackPage | undefined, firstPosition: number, count: number): Array<PlaybackDefinition | undefined> {
	return Array.from({length: count}, (_, index) => page?.assignments[String(firstPosition + index)]);
}
