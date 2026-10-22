import { useLayoutEffect, useRef, useState, type UIEvent } from "react";

/** Reference-axis scrolling is local view state and participates in touch-target placement. */
export function useCueReferenceScroll(
	clipId: string,
	cueId: string | undefined,
) {
	const referenceRail = useRef<HTMLSpanElement>(null);
	const [referenceScroll, setReferenceScroll] = useState(0);
	useLayoutEffect(() => {
		if (referenceRail.current) referenceRail.current.scrollLeft = 0;
		setReferenceScroll(0);
	}, [clipId, cueId]);
	const onReferenceScroll = (event: UIEvent<HTMLSpanElement>) =>
		setReferenceScroll(event.currentTarget.scrollLeft);
	return { referenceRail, referenceScroll, onReferenceScroll };
}
