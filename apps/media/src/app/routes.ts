// The route table.
//
// Routing is Media's own, deliberately: the server serves the shell for any unknown path, so a
// reload on `/layers` works, and the client only has to map a path to a page.

export const ROUTES = [
	{ path: "/", label: "Dashboard" },
	{ path: "/layers", label: "Layers" },
	{ path: "/library", label: "Media library" },
	{ path: "/visualizers", label: "Visualizers" },
	{ path: "/text", label: "Text" },
	{ path: "/audio", label: "Audio" },
	{ path: "/dmx", label: "DMX" },
	{ path: "/settings", label: "Settings" },
	{ path: "/logs", label: "Log" },
] as const;

export type RoutePath = (typeof ROUTES)[number]["path"];

export function normalizePath(pathname: string): RoutePath {
	const trimmed = pathname.replace(/\/+$/u, "") || "/";
	const match = ROUTES.find((route) => route.path === trimmed);
	return match ? match.path : "/";
}
