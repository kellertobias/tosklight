import { useLayoutEffect, useRef, useState } from "react";

/** Reserve scroll extent before the inspector is removed, including Settings changes. */
export function useTimelineInspectorVisibility(
	requested: boolean,
	onChange: ((visible: boolean) => void) | undefined,
	prepareClose: () => void,
) {
	const [visible, setVisible] = useState(requested);
	const previous = useRef(requested);
	useLayoutEffect(() => {
		if (requested === previous.current) return;
		previous.current = requested;
		if (!requested && visible) prepareClose();
		setVisible(requested);
	}, [requested, visible, prepareClose]);
	return {
		visible,
		close() { prepareClose(); setVisible(false); onChange?.(false); },
	};
}
