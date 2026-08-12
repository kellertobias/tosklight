// The route table.
//
// Routing is Media's own, deliberately: the server serves the shell for any unknown path, so a
// reload on `/layers` works, and the client only has to map a path to a page.

export const ROUTES = [
	{ path: "/", label: "Dashboard" },
	{ path: "/media", label: "Playback" },
	{ path: "/library", label: "Library" },
	{ path: "/visualizers", label: "Visualizers" },
	{ path: "/text", label: "Text" },
	{ path: "/settings", label: "Settings" },
] as const;

export type RoutePath = (typeof ROUTES)[number]["path"];

export function normalizePath(pathname: string): RoutePath {
	const trimmed = pathname.replace(/\/+$/u, "") || "/";
	const legacy = {
		"/layers": "/media",
		"/audio": "/settings",
		"/dmx": "/settings",
		"/logs": "/settings",
	} as const;
	const canonical = legacy[trimmed as keyof typeof legacy] ?? trimmed;
	const match = ROUTES.find((route) => route.path === canonical);
	return match ? match.path : "/";
}
