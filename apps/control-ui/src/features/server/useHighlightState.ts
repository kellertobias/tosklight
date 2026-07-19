import { useRef, useState } from "react";
import type { HighlightState } from "../../api/types";

export function useHighlightState() {
	const [highlight, setHighlight] = useState<HighlightState | null>(null);
	const [highlightError, setHighlightError] = useState<string | null>(null);
	const highlightEpoch = useRef(0);
	const highlightWrite = useRef<Promise<unknown>>(Promise.resolve());
	const highlightErrorSticky = useRef(false);
	return {
		highlight,
		setHighlight,
		highlightError,
		setHighlightError,
		highlightEpoch,
		highlightWrite,
		highlightErrorSticky,
	};
}
