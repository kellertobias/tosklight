import type { ShowEntry } from "../../api/types";
import type { ServerController } from "./model";
import type { ServerCapabilities } from "./capabilityContracts";

type ShowLifecycleActions = Pick<
	ServerCapabilities,
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

const SHOW_LOADING_DETAIL =
	"Installing the show engine snapshot and preparing control surfaces";

async function whileLoadingShow<T>(
	model: ServerController,
	title: string,
	task: () => Promise<T>,
): Promise<T> {
	const operationId = model.beginDeskLoading(title, SHOW_LOADING_DETAIL);
	try {
		return await task();
	} finally {
		model.finishDeskLoading(operationId);
	}
}

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
	const { api, setError, bootstrap, shows, setShows, refresh } = model;
	return {
		createShow: async (name) => {
			try {
				await api.shows.createShow(name);
				setShows(await api.shows.shows());
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
					created = await api.shows.renameShow(bootstrap.active_show.id, name);
					shouldOpen = false;
				} else if (bootstrap?.active_show) {
					const blob = await api.shows.downloadShow(bootstrap.active_show.id);
					const bytes = new Uint8Array(await blob.arrayBuffer());
					let binary = "";
					for (const byte of bytes) binary += String.fromCharCode(byte);
					created = await api.shows.createShow(name, btoa(binary), false);
				} else created = await api.shows.createShow(name);
				if (shouldOpen)
					await whileLoadingShow(model, `Loading show ${name}…`, async () => {
						await api.shows.openShow(created.id, "hold_current");
						await refresh();
					});
				else await refresh();
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
				await api.shows.overwriteShow(bootstrap.active_show.id, destination.id);
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
				await whileLoadingShow(model, `Initializing show ${name}…`, async () => {
					const created = await api.shows.createShow(name);
					await api.shows.openShow(created.id, "hold_current");
					await refresh();
				});
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
	const { api, setError, shows, setShows, refresh } = model;
	return {
		uploadShow: async (file, overwrite = false) => {
			try {
				const bytes = new Uint8Array(await file.arrayBuffer());
				let binary = "";
				for (const byte of bytes) binary += String.fromCharCode(byte);
				await api.shows.createShow(
					file.name.replace(/\.show$/i, ""),
					btoa(binary),
					overwrite,
				);
				setShows(await api.shows.shows());
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		openShow: async (id, transition = "safe_blackout") => {
			try {
				const showName = shows.find((show) => show.id === id)?.name;
				await whileLoadingShow(
					model,
					showName ? `Loading show ${showName}…` : "Loading show…",
					async () => {
						await api.shows.openShow(id, transition);
						await refresh();
					},
				);
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		openCleanDefaultShow: async () => {
			try {
				await whileLoadingShow(
					model,
					"Loading clean built-in show…",
					async () => {
						await api.shows.openCleanDefaultShow();
						await refresh();
					},
				);
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
				await whileLoadingShow(model, `Loading show ${showName}…`, async () => {
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
						const blob = await api.files.fileContent(rootId, path);
						const bytes = new Uint8Array(await blob.arrayBuffer());
						let binary = "";
						for (const byte of bytes) binary += String.fromCharCode(byte);
						entry = await api.shows.createShow(showName, btoa(binary), false);
					}
					await api.shows.openShow(entry.id, "safe_blackout");
					await refresh();
				});
				setError(null);
				return true;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				return false;
			}
		},
	};
}
