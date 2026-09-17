// The stubbed server-wide settings routes: the UTC offset, the clip switch hold, the log level,
// and the log feed.
//
// They live beside the stub rather than inside its one fetch handler so the handler stays a
// readable route list.

import type { DataFolderListingView } from "../shared/api/generated/media-wire";
import type { StubbedServer } from "./server";
import { jsonResponse } from "./server";

/// The server-wide settings routes: the UTC offset and the log level, plus the log feed itself.
export function settingsRoute(
	server: StubbedServer,
	path: string,
	init: RequestInit | undefined,
): Response | undefined {
	const body = () => JSON.parse(String(init?.body ?? "{}"));
	if (path.startsWith("/runtime/data-directory/folders")) {
		const directory =
			new URL(path, "http://media.test").searchParams.get("directory") ??
			server.runtime.dataDirectory ??
			"/";
		const listing = server.dataFolders[directory];
		return listing
			? jsonResponse(listing)
			: jsonResponse(
					{ code: "data-folder-invalid", message: `${directory} is gone.` },
					422,
				);
	}
	if (path === "/runtime/data-directory/update") {
		const { directory } = body();
		return jsonResponse({
			directory,
			loadedExisting: Boolean(server.dataFolders[directory]?.hasConfiguration),
			restarting: true,
		});
	}
	if (path === "/time") return jsonResponse(server.time);
	if (path === "/time/update") {
		const { utcOffsetMinutes } = body();
		if (utcOffsetMinutes !== undefined)
			server.time = { ...server.time, utcOffsetMinutes };
		return jsonResponse(server.time);
	}
	if (path === "/playback") return jsonResponse(server.playback);
	if (path === "/playback/update") {
		const { switchHoldMillis, frameRate } = body();
		if (switchHoldMillis !== undefined)
			server.playback = { ...server.playback, switchHoldMillis };
		if (frameRate !== undefined) {
			server.playback = { ...server.playback, frameRate };
			// The rate is server-wide; every output advertises it for its In/Out points.
			for (const output of server.outputs) output.frameRate = frameRate;
		}
		return jsonResponse(server.playback);
	}
	if (path === "/logs/level") return jsonResponse(server.serverLogLevel);
	if (path === "/logs/level/update") {
		server.serverLogLevel.level = body().level;
		return jsonResponse(server.serverLogLevel);
	}
	if (path.startsWith("/logs")) return jsonResponse(server.logs);
	return undefined;
}

/// The Media Server computer's folders as the picker sees them.
export function aDataFolderTree(): Record<string, DataFolderListingView> {
	const folder = (directory: string, hasConfiguration = false) => ({
		name: directory.split("/").at(-1) ?? directory,
		directory,
		hasConfiguration,
	});
	return {
		"/Users/Shared/ToskLight Pixel": {
			directory: "/Users/Shared/ToskLight Pixel",
			parent: "/Users/Shared",
			hasConfiguration: true,
			folders: [folder("/Users/Shared/ToskLight Pixel/Media")],
		},
		"/Users/Shared": {
			directory: "/Users/Shared",
			parent: "/Users",
			hasConfiguration: false,
			folders: [
				folder("/Users/Shared/Autumn Gala", true),
				folder("/Users/Shared/ToskLight Pixel", true),
			],
		},
		"/Users/Shared/Autumn Gala": {
			directory: "/Users/Shared/Autumn Gala",
			parent: "/Users/Shared",
			hasConfiguration: true,
			folders: [],
		},
	};
}
