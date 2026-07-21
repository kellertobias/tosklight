export const MAX_PLAYBACK_PAGE_NAME_CHARACTERS = 80;

export function normalizePlaybackPageName(value: string): string | null {
	const name = value.trim();
	if (
		name.length === 0 ||
		Array.from(name).length > MAX_PLAYBACK_PAGE_NAME_CHARACTERS
	)
		return null;
	return name;
}
