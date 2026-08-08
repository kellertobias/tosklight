// The application shell: navigation, the connection indicator, and the current page.
//
// Pages compose features. Nothing here converts a protocol or holds a state machine.

import { DashboardPage } from "../features/dashboard/DashboardPage";
import { DmxPage } from "../features/dmx/DmxPage";
import { LayersPage } from "../features/layers/LayersPage";
import { LibraryPage } from "../features/media-library/LibraryPage";
import { SettingsPage } from "../features/settings/SettingsPage";
import { VisualizersPage } from "../features/visualizers/VisualizersPage";
import { useHealth } from "../shared/api/queries";
import { ErrorBoundary } from "./ErrorBoundary";
import { ROUTES, type RoutePath } from "./routes";
import { useRouter } from "./useRouter";

// The connection indicator is the only thing on the shell that must stay live on every page.
const HEALTH_POLL_MS = 5_000;

const PAGES: Record<RoutePath, () => React.ReactElement> = {
	"/": DashboardPage,
	"/layers": LayersPage,
	"/library": LibraryPage,
	"/visualizers": VisualizersPage,
	"/dmx": DmxPage,
	"/settings": SettingsPage,
};

export function App() {
	const { path, navigate, headingRef } = useRouter();
	const health = useHealth(HEALTH_POLL_MS);
	const Page = PAGES[path];
	const label = ROUTES.find((route) => route.path === path)?.label ?? "Dashboard";

	return (
		<div className="media-shell">
			<header className="media-shell-header">
				<span className="media-brand">ToskLight Media</span>
				<nav aria-label="Sections">
					{ROUTES.map((route) => (
						<a
							key={route.path}
							href={route.path}
							aria-current={route.path === path ? "page" : undefined}
							onClick={(event) => {
								// Plain clicks route in place; modified clicks stay the browser's.
								if (event.metaKey || event.ctrlKey || event.shiftKey) return;
								event.preventDefault();
								navigate(route.path);
							}}
						>
							{route.label}
						</a>
					))}
				</nav>
				<ConnectionIndicator
					connected={health.data !== undefined && health.failure === undefined}
					instance={health.data?.instance}
				/>
			</header>

			<main className="media-shell-main">
				<h1 tabIndex={-1} ref={headingRef}>
					{label}
				</h1>
				<ErrorBoundary key={path}>
					<Page />
				</ErrorBoundary>
			</main>
		</div>
	);
}

function ConnectionIndicator({
	connected,
	instance,
}: {
	connected: boolean;
	instance: string | undefined;
}) {
	return (
		<p className={`media-connection ${connected ? "is-connected" : "is-disconnected"}`} role="status">
			<span aria-hidden="true">●</span>{" "}
			{connected ? `Connected${instance ? ` · ${instance}` : ""}` : "Not connected"}
		</p>
	);
}
