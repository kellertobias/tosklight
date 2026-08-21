// The stubbed server-wide settings routes: the UTC offset, the log level, and the log feed.
//
// They live beside the stub rather than inside its one fetch handler so the handler stays a
// readable route list.

import type { StubbedServer } from "./server";
import { jsonResponse } from "./server";

/// The server-wide settings routes: the UTC offset and the log level, plus the log feed itself.
export function settingsRoute(
	server: StubbedServer,
	path: string,
	init: RequestInit | undefined,
): Response | undefined {
	const body = () => JSON.parse(String(init?.body ?? "{}"));
	if (path === "/time") return jsonResponse(server.time);
	if (path === "/time/update") {
		const { utcOffsetMinutes } = body();
		if (utcOffsetMinutes !== undefined)
			server.time = { ...server.time, utcOffsetMinutes };
		return jsonResponse(server.time);
	}
	if (path === "/logs/level") return jsonResponse(server.serverLogLevel);
	if (path === "/logs/level/update") {
		server.serverLogLevel.level = body().level;
		return jsonResponse(server.serverLogLevel);
	}
	if (path.startsWith("/logs")) return jsonResponse(server.logs);
	return undefined;
}

