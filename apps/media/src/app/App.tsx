// The application shell: navigation, the connection indicator, and the current page.
//
// Pages compose features. Nothing here converts a protocol or holds a state machine.

import { useEffect, useState } from "react";
import { DashboardPage } from "../features/dashboard/DashboardPage";
import { LayersPage } from "../features/layers/LayersPage";
import { LibraryPage } from "../features/media-library/LibraryPage";
import { SettingsPage } from "../features/settings/SettingsPage";
import { TextSourcesPage } from "../features/text-sources/TextSourcesPage";
import { VisualizersPage } from "../features/visualizers/VisualizersPage";
import {
	MediaScreenHeader,
	type MediaServerSection,
	MediaServerShell,
} from "../operator/MediaServerSurface";
import { useHealth } from "../shared/api/queries";
import { ErrorBoundary } from "./ErrorBoundary";
import { ROUTES, type RoutePath } from "./routes";
import { useRouter } from "./useRouter";

// The connection indicator is the only thing on the shell that must stay live on every page.
const HEALTH_POLL_MS = 5_000;

const PAGES: Record<RoutePath, () => React.ReactElement> = {
	"/": DashboardPage,
	"/media": LayersPage,
	"/library": LibraryPage,
	"/visualizers": VisualizersPage,
	"/text": TextSourcesPage,
	"/settings": SettingsPage,
};

const SECTION_BY_PATH: Record<RoutePath, MediaServerSection> = {
	"/": "dashboard",
	"/media": "media",
	"/library": "library",
	"/visualizers": "visualizers",
	"/text": "text",
	"/settings": "settings",
};

const DETAIL_BY_PATH: Record<RoutePath, string> = {
	"/": "See the server, desk, output, and library state that matters during a show.",
	"/media":
		"See the source and level currently resolved on every output layer.",
	"/library":
		"Prepare stable numbered media without changing the address a show uses.",
	"/visualizers":
		"Select a generated source to inspect its look and tune its controls.",
	"/text": "Write and style addressable text, clocks, and countdowns.",
	"/settings": "Configure this Media Server through focused operator settings.",
};

export function App() {
	const { path, navigate, headingRef } = useRouter();
	const health = useHealth(HEALTH_POLL_MS);
	const Page = PAGES[path];
	const label =
		ROUTES.find((route) => route.path === path)?.label ?? "Dashboard";
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
			now={now}
			onNavigate={(section) => {
				const route = ROUTES.find(
					(candidate) => SECTION_BY_PATH[candidate.path] === section,
				);
				if (route) navigate(route.path);
			}}
		>
			<div ref={headingRef} tabIndex={-1}>
				<MediaScreenHeader title={label} detail={DETAIL_BY_PATH[path]} />
				<ErrorBoundary key={path}>
					<Page />
				</ErrorBoundary>
			</div>
		</MediaServerShell>
	);
}
