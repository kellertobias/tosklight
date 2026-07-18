import type { ShowEntry } from "../../api/types";
import type { ServerController } from "./model";
import type { ServerContextValue } from "./ServerContextValue";

type ShowLifecycleActions = Pick<
	ServerContextValue,
	| "createShow"
	| "saveShowAs"
	| "overwriteShow"
	| "initializeEmptyShow"
	| "uploadShow"
	| "openShow"
	| "openCleanDefaultShow"
	| "openShowFile"
>;

type ShowCreationActions = Pick<
	ShowLifecycleActions,
	"createShow" | "saveShowAs" | "overwriteShow" | "initializeEmptyShow"
>;

type ShowOpeningActions = Omit<ShowLifecycleActions, keyof ShowCreationActions>;

export function createShowLifecycleActions(
	model: ServerController,
): ShowLifecycleActions {
	return {
		...createShowCreationActions(model),
		...createShowOpeningActions(model),
	};
}

function createShowCreationActions(
	model: ServerController,
): ShowCreationActions {
	const { client, setError, bootstrap, shows, setShows, refresh } = model;
	return {
		createShow: async (name) => {
			try {
				await client.createShow(name);
				setShows(await client.shows());
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		saveShowAs: async (name) => {
			try {
				let created: ShowEntry;
				let shouldOpen = true;
				if (
					bootstrap?.active_show &&
					/^New Empty Show(?: [1-9]\d*)?$/.test(bootstrap.active_show.name)
				) {
					created = await client.renameShow(bootstrap.active_show.id, name);
					shouldOpen = false;
				} else if (bootstrap?.active_show) {
					const blob = await client.downloadShow(bootstrap.active_show.id);
					const bytes = new Uint8Array(await blob.arrayBuffer());
					let binary = "";
					for (const byte of bytes) binary += String.fromCharCode(byte);
					created = await client.createShow(name, btoa(binary), false);
				} else created = await client.createShow(name);
				if (shouldOpen) await client.openShow(created.id, "hold_current");
				await refresh();
				setError(null);
				return true;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				return false;
			}
		},
		overwriteShow: async (destinationId) => {
			try {
				if (!bootstrap?.active_show)
					throw new Error(
						"Open a show before choosing an overwrite destination",
					);
				if (bootstrap.active_show.id === destinationId)
					throw new Error("The active show is already that destination");
				const destination = shows.find((show) => show.id === destinationId);
				if (!destination)
					throw new Error("The overwrite destination is no longer available");
				await client.overwriteShow(bootstrap.active_show.id, destination.id);
				await refresh();
				setError(null);
				return true;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				return false;
			}
		},
		initializeEmptyShow: async () => {
			try {
				const names = new Set(shows.map((show) => show.name.toLowerCase()));
				let name = "New Empty Show";
				for (let suffix = 2; names.has(name.toLowerCase()); suffix += 1)
					name = `New Empty Show ${suffix}`;
				const created = await client.createShow(name);
				await client.openShow(created.id, "hold_current");
				await refresh();
				setError(null);
				return true;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				return false;
			}
		},
	};
}

function createShowOpeningActions(model: ServerController): ShowOpeningActions {
	const { client, setError, shows, setShows, refresh } = model;
	return {
		uploadShow: async (file, overwrite = false) => {
			try {
				const bytes = new Uint8Array(await file.arrayBuffer());
				let binary = "";
				for (const byte of bytes) binary += String.fromCharCode(byte);
				await client.createShow(
					file.name.replace(/\.show$/i, ""),
					btoa(binary),
					overwrite,
				);
				setShows(await client.shows());
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		openShow: async (id, transition = "safe_blackout") => {
			try {
				await client.openShow(id, transition);
				await refresh();
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		openCleanDefaultShow: async () => {
			try {
				await client.openCleanDefaultShow();
				await refresh();
				setError(null);
				return true;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				return false;
			}
		},
		openShowFile: async (rootId, path, name) => {
			try {
				const showName = name.replace(/\.show$/i, "");
				let entry =
					rootId === "shows"
						? shows.find(
								(show) =>
									show.name.localeCompare(showName, undefined, {
										sensitivity: "accent",
									}) === 0,
							)
						: undefined;
				if (!entry) {
					const blob = await client.fileContent(rootId, path);
					const bytes = new Uint8Array(await blob.arrayBuffer());
					let binary = "";
					for (const byte of bytes) binary += String.fromCharCode(byte);
					entry = await client.createShow(showName, btoa(binary), false);
				}
				await client.openShow(entry.id, "safe_blackout");
				await refresh();
				setError(null);
				return true;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				return false;
			}
		},
	};
}
