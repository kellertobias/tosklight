import {
	type CSSProperties,
	type MutableRefObject,
	useEffect,
	useRef,
	useState,
} from "react";
import { ServerRuntime } from "./api/ServerRuntime";
import type { ScreenConfiguration } from "./api/types";
import { LoadingSurface } from "./components/common/LoadingSurface";
import { DeskLockOverlay } from "./components/modals/DeskLockOverlay";
import { ConnectionState } from "./components/shell/ConnectionState";
import { DeskLoadingOverlay } from "./components/shell/DeskLoadingOverlay";
import { LeftDock } from "./components/shell/LeftDock";
import { NativeDragStrip } from "./components/shell/NativeDragStrip";
import { WorkspaceView } from "./components/shell/WorkspaceView";
import { PatchFeatureBoundary } from "./features/patch/PatchFeatureBoundary";
import { screenShowsPlaybacks } from "./features/screens/encoderPlacement";
import { FixedScreenPane } from "./features/screens/FixedScreenPane";
import { ScreenControlRegion } from "./features/screens/ScreenControlRegion";
import { useScreens } from "./features/screens/ScreensContext";
import { useScreenWindowPersistence } from "./platform/desktop";
import { AppProvider, useApp } from "./state/AppContext";

function DesktopScreenSurface({
	screen,
	saveScreen,
	closing,
}: {
	screen: ScreenConfiguration;
	saveScreen: (screen: ScreenConfiguration) => Promise<void>;
	closing: MutableRefObject<boolean>;
}) {
	const { state, dispatch } = useApp();
	const programmerOwner =
		useScreens().screens?.programmer_control_surface?.owner_screen_id ===
		screen.id;
	const showScreenControls = screenShowsPlaybacks(screen);
	const hydrated = useRef(false);
	const [layoutReady, setLayoutReady] = useState(false);
	const screenRef = useRef(screen);
	screenRef.current = screen;
	useEffect(() => {
		if (hydrated.current) return;
		setLayoutReady(false);
		dispatch({
			type: "HYDRATE_LAYOUT",
			desks: screen.layout.desks,
			activeDeskId: screen.layout.activeDeskId,
		});
		hydrated.current = true;
		setLayoutReady(true);
	}, [screen, dispatch]);
	useEffect(() => {
		const currentScreen = screenRef.current;
		if (!currentScreen || !hydrated.current || closing.current) return;
		const timer = window.setTimeout(() => {
			const latest = screenRef.current;
			if (latest && !closing.current)
				void saveScreen({
					...latest,
					layout: { desks: state.desks, activeDeskId: state.activeDeskId },
				});
		}, 600);
		return () => window.clearTimeout(timer);
	}, [state.desks, state.activeDeskId]);
	if (!layoutReady || !hydrated.current)
		return (
			<LoadingSurface
				className="connection-cover"
				showMark
				title="Loading screen…"
				detail="Hydrating the assigned desktop and Playback surface"
			/>
		);
	return (
		<div
			className={`screen-shell ${screen.show_dock ? "with-dock" : ""} ${showScreenControls ? "with-playbacks" : ""} ${programmerOwner ? "with-control" : ""}`}
		>
			<NativeDragStrip />
			{screen.show_dock && <LeftDock />}
			<WorkspaceView />
			{(showScreenControls || programmerOwner) && (
				<ScreenControlRegion screen={screen} />
			)}
		</div>
	);
}

function FixedScreenSurface({ screen }: { screen: ScreenConfiguration }) {
	const programmerOwner =
		useScreens().screens?.programmer_control_surface?.owner_screen_id ===
		screen.id;
	if (screen.content.type !== "fixed_pane") return null;
	const showScreenControls = screenShowsPlaybacks(screen);
	return (
		<div
			className={`screen-shell fixed-content ${showScreenControls ? "with-playbacks" : ""} ${programmerOwner ? "with-control" : ""}`}
		>
			<NativeDragStrip />
			<FixedScreenPane pane={screen.content.pane} />
			{(showScreenControls || programmerOwner) && (
				<ScreenControlRegion screen={screen} />
			)}
		</div>
	);
}

/**
 * A fixed side pane divides the whole screen: the widget keeps its configured width
 * over the full height and the control region takes every remaining pixel, so nothing
 * is reserved for an empty region above it.
 */
function SideScreenSurface({ screen }: { screen: ScreenConfiguration }) {
	if (screen.content.type !== "fixed_side_pane") return null;
	const { side, width_percent, pane } = screen.content;
	return (
		<div
			className={`screen-shell side-content fixed-${side}`}
			style={
				{ "--fixed-side-pane-width": `${width_percent}%` } as CSSProperties
			}
		>
			<NativeDragStrip />
			<FixedScreenPane pane={pane} />
			<div className="screen-side-base">
				<ScreenControlRegion screen={screen} />
			</div>
		</div>
	);
}

/** Controls only: the control region owns the full screen height. */
function ControlScreenSurface({ screen }: { screen: ScreenConfiguration }) {
	return (
		<div className="screen-shell utility-content">
			<NativeDragStrip />
			<ScreenControlRegion screen={screen} />
		</div>
	);
}

function ScreenSurface({ id }: { id: string }) {
	const server = useScreens();
	const screen = server.screens?.screens.find((item) => item.id === id);
	const closing = useScreenWindowPersistence(screen, server.saveScreen);
	if (server.screens && !screen)
		return (
			<div className="connection-cover parameter-empty" role="alert">
				<b>Screen unavailable</b>
				<small>
					This screen was removed or this browser link is not authorized for
					the current desk.
				</small>
			</div>
		);
	if (!screen)
		return (
			<LoadingSurface
				className="connection-cover"
				showMark
				title="Loading screen…"
				detail="Waiting for the assigned screen configuration"
			/>
		);
	if (screen.content.type === "fixed_pane")
		return <FixedScreenSurface screen={screen} />;
	if (screen.content.type === "fixed_side_pane")
		return <SideScreenSurface screen={screen} />;
	if (screen.content.type === "control_surface")
		return <ControlScreenSurface screen={screen} />;
	return (
		<DesktopScreenSurface
			screen={screen}
			saveScreen={server.saveScreen}
			closing={closing}
		/>
	);
}

export function ScreenApp({ id }: { id: string }) {
	return (
		<ServerRuntime sessionRole="secondary">
			<AppProvider>
				<PatchFeatureBoundary>
					<ScreenSurface id={id} />
					<ConnectionState />
					<DeskLoadingOverlay />
				</PatchFeatureBoundary>
			</AppProvider>
			<DeskLockOverlay />
		</ServerRuntime>
	);
}
