/** Derive Main's minimum footer from authored track proportions and actual controls. */
export function requiredMainPlaybackHeight(bank: HTMLElement) {
	const style = getComputedStyle(bank);
	const tracks = style.gridTemplateRows.split(" ").map(Number.parseFloat);
	const trackHeight = tracks.reduce((sum, height) => sum + height, 0);
	if (!trackHeight || tracks.some((height) => !Number.isFinite(height))) return 0;
	let required = 0;
	for (const card of bank.querySelectorAll<HTMLElement>("article")) {
		const box = card.getBoundingClientRect();
		const cardStyle = getComputedStyle(card);
		const stack = card.querySelector<HTMLElement>(".vertical-touch-fader-stack");
		const actions = stack?.querySelector<HTMLElement>(
			".vertical-touch-fader-actions",
		);
		const inset =
			Number.parseFloat(cardStyle.paddingBottom) +
			Number.parseFloat(cardStyle.borderBottomWidth);
		const minimum = stack
			? stack.getBoundingClientRect().top - box.top + 44 + inset +
				(actions
					? actions.getBoundingClientRect().height +
						Number.parseFloat(getComputedStyle(stack).rowGap)
					: 0)
			: 44 + inset +
				Number.parseFloat(cardStyle.paddingTop) +
				Number.parseFloat(cardStyle.borderTopWidth);
		const weight = box.height / trackHeight;
		if (weight > 0) required = Math.max(required, minimum / weight);
	}
	const bankBounds = bank.getBoundingClientRect();
	const footer = bank.closest<HTMLElement>(".control-section");
	if (!footer) return 0;
	const gaps = Number.parseFloat(style.rowGap) * Math.max(0, tracks.length - 1);
	const padding = Number.parseFloat(style.paddingTop) + Number.parseFloat(style.paddingBottom);
	const chrome = footer.getBoundingClientRect().height - bankBounds.height;
	// The existing app shell reserves control-height minus its 21px native-chrome allowance.
	return Math.ceil(required + gaps + padding + chrome + 21);
}
