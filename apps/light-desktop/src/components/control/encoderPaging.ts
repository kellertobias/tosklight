/**
 * Fitting a view's encoder slots onto the encoders a desk actually has.
 *
 * A surface carries four or six encoders depending on how the desk is set up, and a view does not
 * know which. The Timecode editor's Cue deck, for instance, offers seven: four Cue timings, the
 * Cue itself, and the two the timeline always keeps. Rendering all of them puts controls where
 * there is no encoder to turn.
 *
 * So a deck that does not fit is paged, and the page count follows the surface rather than the
 * view. Nothing is dropped and nothing is resized: the operator turns the page instead.
 */

/** One page of slots, and where it sits in the run. */
export interface EncoderPage<Slot> {
	slots: Slot[];
	/** 1-based, for "1/2". */
	number: number;
	total: number;
}

/**
 * Split `slots` across pages of `visibleEncoders`.
 *
 * A deck that fits is one page, which is the ordinary case and reads no differently to an
 * operator: a single page shows no page controls.
 */
export function encoderPages<Slot>(
	slots: readonly Slot[],
	visibleEncoders: number,
): EncoderPage<Slot>[] {
	// A surface that reports no encoders still has to show the deck somewhere rather than
	// dividing by zero and showing nothing.
	const perPage = Math.max(1, Math.floor(visibleEncoders));
	if (slots.length === 0) {
		return [{ slots: [], number: 1, total: 1 }];
	}
	const total = Math.ceil(slots.length / perPage);
	return Array.from({ length: total }, (_, index) => ({
		slots: slots.slice(index * perPage, (index + 1) * perPage),
		number: index + 1,
		total,
	}));
}

/**
 * Keep a page number inside a deck that has changed under it.
 *
 * Selecting a different object can shorten the deck while the operator is on its last page, and
 * being left on a page that no longer exists would show nothing at all.
 */
export function clampEncoderPage(page: number, total: number): number {
	return Math.min(Math.max(1, page), Math.max(1, total));
}
