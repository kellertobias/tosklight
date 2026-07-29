export const PHYSICAL_PLAYBACK_MAX = 1_000;
export const VIRTUAL_PLAYBACKS_PER_PAGE = 300;
export const MAX_PLAYBACK_PAGE = 127;
export const MIN_VIRTUAL_PLAYBACK_NUMBER = PHYSICAL_PLAYBACK_MAX + 1;
export const MAX_VIRTUAL_PLAYBACK_NUMBER =
	PHYSICAL_PLAYBACK_MAX + VIRTUAL_PLAYBACKS_PER_PAGE * MAX_PLAYBACK_PAGE;

export function virtualPlaybackBankStart(page: number) {
	return (
		MIN_VIRTUAL_PLAYBACK_NUMBER +
		(page - 1) * VIRTUAL_PLAYBACKS_PER_PAGE
	);
}

export function virtualPlaybackNumber(page: number, cell: number) {
	return virtualPlaybackBankStart(page) + cell - 1;
}

export function virtualPlaybackPage(number: number) {
	if (
		!Number.isSafeInteger(number) ||
		number < MIN_VIRTUAL_PLAYBACK_NUMBER ||
		number > MAX_VIRTUAL_PLAYBACK_NUMBER
	)
		return null;
	return (
		Math.floor(
			(number - MIN_VIRTUAL_PLAYBACK_NUMBER) / VIRTUAL_PLAYBACKS_PER_PAGE,
		) + 1
	);
}

export function isVirtualPlaybackNumberForPage(page: number, number: number) {
	return (
		Number.isSafeInteger(page) &&
		page >= 1 &&
		page <= MAX_PLAYBACK_PAGE &&
		virtualPlaybackPage(number) === page
	);
}
