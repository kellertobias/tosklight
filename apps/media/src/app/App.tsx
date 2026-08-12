// The application shell: navigation, the connection indicator, and the current page.
//
// Pages compose features. Nothing here converts a protocol or holds a state machine.

import { WindowFrame } from "@tosklight/ui/window-kit";
import { useEffect, useState } from "react";
import { AudioPage } from "../features/audio/AudioPage";
import { DashboardPage } from "../features/dashboard/DashboardPage";
import { MediaPanePage } from "../features/layers/MediaPanePage";
import { LibraryPage } from "../features/media-library/LibraryPage";
import { SettingsPage } from "../features/settings/SettingsPage";
import { DeskIdentityProvider } from "../operator/DeskIdentityContext";
import {
	type MediaServerSection,
	MediaServerShell,
} from "../operator/MediaServerSurface";
import { useHealth } from "../shared/api/queries";
import { useTelemetry } from "../shared/api/telemetry";
import { ErrorBoundary } from "./ErrorBoundary";
import { ROUTES, type RoutePath } from "./routes";
import { useRouter } from "./useRouter";

// The connection indicator is the only thing on the shell that must stay live on every page.
const HEALTH_POLL_MS = 5_000;

const PAGES: Record<RoutePath, () => React.ReactElement> = {
	"/": DashboardPage,
	"/media": MediaPanePage,
	"/library": () => <LibraryPage />,
	"/visualizers": () => <LibraryPage mode="visualizers" />,
	"/text": () => <LibraryPage mode="text" />,
	"/audio": AudioPage,
	"/settings": SettingsPage,
};

const SECTION_BY_PATH: Record<RoutePath, MediaServerSection> = {
	"/": "dashboard",
	"/media": "media",
	"/library": "library",
	"/visualizers": "library",
	"/text": "library",
	"/audio": "audio",
	"/settings": "settings",
};

export function App() {
	const { path, navigate, headingRef } = useRouter();
	const health = useHealth(HEALTH_POLL_MS);
	const telemetry = useTelemetry();
	const showName = telemetry.frame?.deskIdentity?.showName;
	const Page = PAGES[path];
	const route =
		ROUTES.find((candidate) => candidate.path === path) ?? ROUTES[0];
	const pageOwnsWindow =
		path === "/media" ||
		path === "/library" ||
		path === "/visualizers" ||
		path === "/text" ||
		path === "/settings";
	const libraryMode =
		path === "/visualizers"
			? "visualizers"
			: path === "/text"
				? "text"
				: "media";
	const [now, setNow] = useState(() => new Date());
	useEffect(() => {
		const timer = window.setInterval(() => setNow(new Date()), 30_000);
		return () => window.clearInterval(timer);
	}, []);

	return (
		<MediaServerShell
			active={SECTION_BY_PATH[path]}
			connected={health.data !== undefined && health.failure === undefined}
			instance={health.data?.instance}
			showName={showName}
			now={now}
			onNavigate={(section) => {
				const route = ROUTES.find(
					(candidate) => SECTION_BY_PATH[candidate.path] === section,
				);
				if (route) navigate(route.path);
			}}
		>
			<DeskIdentityProvider showName={showName}>
				<div ref={headingRef} tabIndex={-1} className="media-route-surface">
					<ErrorBoundary key={path}>
						{pageOwnsWindow ? (
							path === "/library" ||
							path === "/visualizers" ||
							path === "/text" ? (
								<LibraryPage
									mode={libraryMode}
									onModeChange={(mode) =>
										navigate(
											mode === "media"
												? "/library"
												: mode === "visualizers"
													? "/visualizers"
													: "/text",
										)
									}
								/>
							) : (
								<Page />
							)
						) : (
							<WindowFrame
								title={route.label}
								info={{
									primary: "Media Server",
									secondary: "Operator controls",
								}}
								className="media-route-window"
							>
								<Page />
							</WindowFrame>
						)}
					</ErrorBoundary>
				</div>
			</DeskIdentityProvider>
		</MediaServerShell>
	);
}
