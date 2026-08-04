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
import { FixedScreenPane } from "./features/screens/FixedScreenPane";
import { ProgrammerControlSurfaceRegion } from "./features/screens/ProgrammerControlSurfaceRegion";
import { ScreenPlaybackSection } from "./features/screens/ScreenPlaybackSection";
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
	const showScreenControls =
		!programmerOwner && (screen.show_playbacks || screen.show_page_controls);
	const hydrated = useRef(false);
	const [layoutReady, setLayoutReady] = useState(false);
	const screenRef = useRef(screen);
	const sidePane =
		screen.content.type === "fixed_side_pane" ? screen.content : null;
	screenRef.current = screen;
	useEffect(() => {
		if (hydrated.current) return;
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
	if (!layoutReady)
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
			{sidePane ? (
				<div
					className={`screen-main-composition fixed-${sidePane.side}`}
					style={
						{
							"--fixed-side-pane-width": `${sidePane.width_px}px`,
						} as CSSProperties
					}
				>
					<FixedScreenPane pane={sidePane.pane} />
					<WorkspaceView />
				</div>
			) : (
				<WorkspaceView />
			)}
			{showScreenControls && <ScreenPlaybackSection screen={screen} />}
			{programmerOwner && (
				<ProgrammerControlSurfaceRegion screenId={screen.id} />
			)}
		</div>
	);
}

function FixedScreenSurface({ screen }: { screen: ScreenConfiguration }) {
	const programmerOwner =
		useScreens().screens?.programmer_control_surface?.owner_screen_id ===
		screen.id;
	if (screen.content.type !== "fixed_pane") return null;
	const showScreenControls =
		!programmerOwner && (screen.show_playbacks || screen.show_page_controls);
	return (
		<div
			className={`screen-shell fixed-content ${showScreenControls ? "with-playbacks" : ""} ${programmerOwner ? "with-control" : ""}`}
		>
			<NativeDragStrip />
			<FixedScreenPane pane={screen.content.pane} />
			{showScreenControls && <ScreenPlaybackSection screen={screen} />}
			{programmerOwner && (
				<ProgrammerControlSurfaceRegion screenId={screen.id} />
			)}
		</div>
	);
}

function ScreenSurface({ id }: { id: string }) {
	const server = useScreens();
	const screen = server.screens?.screens.find((item) => item.id === id);
	const closing = useScreenWindowPersistence(screen, server.saveScreen);
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
