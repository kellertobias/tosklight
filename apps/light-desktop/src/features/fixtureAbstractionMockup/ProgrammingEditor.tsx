import { useLayoutEffect, useRef, useState } from "react";

/** Budget against the actual encoder container, including the unchanged keypad. */
export function useEncoderArea() {
	const ref = useRef<HTMLDivElement>(null);
	const [size, setSize] = useState({ width: 0, height: 0 });
	useLayoutEffect(() => {
		const element = ref.current;
		if (!element) return;
		const observer = new ResizeObserver(([entry]) => setSize({ width: entry.contentRect.width, height: entry.contentRect.height }));
		observer.observe(element);
		return () => observer.disconnect();
	}, []);
	return { ref, fits: size.width >= 680 && size.height >= 210, narrow: size.width > 0 && size.width < 460 };
}

