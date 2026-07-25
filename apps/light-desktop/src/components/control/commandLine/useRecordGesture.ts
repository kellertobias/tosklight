import { useEffect, useRef } from "react";
import { useApp } from "../../../state/AppContext";
import { openUpdateSettings } from "../updateWorkflow";

export function useRecordGesture({
	armUpdateOrMenu,
	toggleRecord,
}: {
	armUpdateOrMenu: () => void;
	toggleRecord: () => void;
}) {
	const { dispatch } = useApp();
	const hold = useRef<number | null>(null);
	const held = useRef(false);
	const suppressUntil = useRef(0);
	const mode = useRef<"record" | "update" | null>(null);
	const end = () => {
		if (hold.current) window.clearTimeout(hold.current);
		hold.current = null;
	};
	const begin = (shifted: boolean) => {
		held.current = false;
		mode.current = shifted ? "update" : "record";
		hold.current = window.setTimeout(() => {
			held.current = true;
			suppressUntil.current = performance.now() + 1000;
			if (mode.current === "update") openUpdateSettings();
			else
				dispatch({
					type: "SET_MODAL",
					modal: "storeSettingsOpen",
					value: true,
				});
		}, 650);
	};
	const cancel = () => {
		end();
		mode.current = null;
	};
	const complete = (shifted: boolean) => {
		const updateGesture = mode.current === "update" || shifted;
		mode.current = null;
		if (held.current || performance.now() < suppressUntil.current) {
			held.current = false;
			return;
		}
		if (updateGesture) armUpdateOrMenu();
		else toggleRecord();
	};
	useEffect(
		() => () => {
			if (hold.current) window.clearTimeout(hold.current);
		},
		[],
	);
	return { begin, end, cancel, complete };
}
