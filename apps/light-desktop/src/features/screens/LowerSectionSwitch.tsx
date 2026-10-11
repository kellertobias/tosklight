import { Button } from "@tosklight/ui";
import { createContext, type ReactNode, useContext, useEffect, useRef } from "react";

import { usePlaybackSectionSettings } from "../../components/control/PlaybackSectionSettingsHost";

export type LowerSectionView = "encoders" | "playbacks";

/**
 * The Playback/Encoders switch does not own a row of its own. The section that is
 * currently visible places it beside its own controls — after the Dynamics button on
 * the encoder tabs, above PAGE UP on the page controls — so neither section loses
 * height to the switch.
 */
const LowerSectionSwitchContext = createContext<ReactNode>(null);

export function LowerSectionSwitchProvider({
	switchNode,
	children,
}: {
	switchNode: ReactNode;
	children: ReactNode;
}) {
	return (
		<LowerSectionSwitchContext.Provider value={switchNode}>
			{children}
		</LowerSectionSwitchContext.Provider>
	);
}

/** Null on every surface that carries only one of the two sections. */
export function useLowerSectionSwitch() {
	return useContext(LowerSectionSwitchContext);
}

export function LowerSectionSwitch({
	view,
	onView,
}: {
	view: LowerSectionView;
	onView: (view: LowerSectionView) => void;
}) {
	const settings = usePlaybackSectionSettings();
	const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
	const suppressClick = useRef(false);
	const clearHold = () => {
		if (timer.current !== null) clearTimeout(timer.current);
		timer.current = null;
	};
	useEffect(() => () => clearHold(), []);
	const cancelHold = () => {
		clearHold();
		suppressClick.current = true;
	};
	/*
	 * One button carrying both labels. Its geometry never changes with the view — only
	 * which label is the active colour — so the row it sits in does not reflow when the
	 * operator switches sections.
	 */
	return (
		<Button
			className="screen-section-switch"
			aria-label="Lower section"
			data-section={view}
			onPointerDown={() => {
				clearHold();
				suppressClick.current = false;
				if (settings) timer.current = setTimeout(() => {
					timer.current = null;
					suppressClick.current = true;
					settings.open();
				}, 650);
			}}
			onPointerUp={clearHold}
			onPointerCancel={cancelHold}
			onPointerLeave={cancelHold}
			onContextMenu={(event) => event.preventDefault()}
			onClick={(event) => {
				if (!suppressClick.current || event.detail === 0)
					onView(view === "encoders" ? "playbacks" : "encoders");
				suppressClick.current = false;
			}}
		>
			<span
				className={view === "playbacks" ? "active" : ""}
				aria-current={view === "playbacks"}
			>
				Playback
			</span>
			<span
				className={view === "encoders" ? "active" : ""}
				aria-current={view === "encoders"}
			>
				Encoders
			</span>
		</Button>
	);
}
