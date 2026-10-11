/** Exact show identity shared by every playback control surface. */
export interface PlaybackAddressValue {
	page: number;
	position: number;
}

export const MAX_PLAYBACK_PAGE = 99;
export const MAX_PLAYBACK_POSITION = 1_000;

export function assertPlaybackAddress(address: PlaybackAddressValue): void {
	if (
		!Number.isSafeInteger(address.page) ||
		address.page < 1 ||
		address.page > MAX_PLAYBACK_PAGE
	)
		throw new Error("Playback page must be between 1 and 99");
	if (
		!Number.isSafeInteger(address.position) ||
		address.position < 1 ||
		address.position > MAX_PLAYBACK_POSITION
	)
		throw new Error("Playback position must be between 1 and 1000");
}

export function playbackAddressKey(address: PlaybackAddressValue): string {
	assertPlaybackAddress(address);
	return `playback:${address.page}.${address.position}`;
}

export function samePlaybackAddress(
	left: PlaybackAddressValue,
	right: PlaybackAddressValue,
): boolean {
	return left.page === right.page && left.position === right.position;
}
