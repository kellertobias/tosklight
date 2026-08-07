import { Button } from "@tosklight/ui";
import { createContext, type ReactNode, useContext } from "react";

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
			onClick={() =>
				onView(view === "encoders" ? "playbacks" : "encoders")
			}
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
