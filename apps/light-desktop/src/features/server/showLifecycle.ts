import { formatErrorDetails } from "@tosklight/ui";
import { ApiRequestError } from "../../api/ApiRequestError";
import type { ShowEntry } from "../../api/types";
import type { ServerController } from "./model";
import type { ServerCapabilities } from "./capabilityContracts";

type ShowLifecycleActions = Pick<
	ServerCapabilities,
	| "networkShows"
    | "setShowDescription"
    | "networkSaveFolders"
    | "saveShowCopy"
    | "exportMvrFile"
    | "importRemoteShow"
    | "prepareShowRevision"
    | "prepareShowFile"
	| "createShow"
	| "saveShowAs"
	| "overwriteShow"
	| "initializeEmptyShow"
	| "uploadShow"
	| "openShow"
	| "openCleanDefaultShow"
	| "openShowFile"
	| "discoveredVisualizers"
	| "loadFromVisualizer"
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
				setError(formatErrorDetails(reason));
			}
		},
		saveShowAs: async (name, options = {}) => {
			try {
				let created: ShowEntry;
				let shouldOpen = true;
				if (options.latest && bootstrap?.active_show) {
                    created = bootstrap.active_show;
                    shouldOpen = false;
                } else if (
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
				created = await api.shows.setBaseShow(created.id, options.baseShow ?? false);
				if (shouldOpen)
					await whileLoadingShow(model, `Loading show ${name}…`, async () => {
						await api.shows.openShow(created.id, "hold_current");
						await refresh();
					});
				else await refresh();
				setError(null);
				return true;
			} catch (reason) {
				setError(formatErrorDetails(reason));
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
				setError(formatErrorDetails(reason));
				return false;
			}
		},
		initializeEmptyShow: async (baseShowId) => {
			try {
				const prefix = baseShowId ? "New Show from Base" : "New Empty Show";
				await whileLoadingShow(model, `Initializing ${prefix.toLowerCase()}…`, async () => {
					for (let attempt = 0; attempt < 3; attempt += 1) {
						let library: ShowEntry[];
						try {
							library = await api.shows.shows();
						} catch (reason) {
							throw new Error(`Could not read the saved show library. Retry creating the empty show. ${formatErrorDetails(reason)}`);
						}
						setShows(library);
						const names = new Set(library.map((show) => show.name.toLowerCase()));
						let name = prefix;
						for (let suffix = 2; names.has(name.toLowerCase()); suffix += 1)
							name = `${prefix} ${suffix}`;
						let created: ShowEntry;
						try {
							created = baseShowId ? await api.shows.createFromBase(baseShowId, name) : await api.shows.createShow(name);
						} catch (reason) {
							// Only a definite name collision permits another creation attempt.
							// Network/unknown failures must not duplicate a possibly created show.
							if (reason instanceof ApiRequestError && (reason.status === 400 || reason.status === 409)
								&& reason.message.toLowerCase().includes("a show with that name already exists")) continue;
							throw reason;
						}
						await api.shows.openShow(created.id, "hold_current");
						await refresh();
						return;
					}
					throw new Error("Another desk is creating shows with the same names. Retry creating the empty show.");
				});
				setError(null);
				return true;
			} catch (reason) {
				setError(formatErrorDetails(reason));
				return false;
			}
		},
	};
}

function createShowOpeningActions(model: ServerController): ShowOpeningActions {
	const { api, setError, shows, setShows, refresh } = model;
	return {
        setShowDescription: async (id,description) => {
            await api.shows.setDescription(id,description);
            setShows(await api.shows.shows());
        },
        networkSaveFolders: (instance, rootId, path) => api.shows.networkSaveFolders(instance, rootId, path),
        saveShowCopy: async (name, target, baseShow) => {
            const source = model.bootstrap?.active_show;
            if (!source) throw new Error("Open a show before saving a copy");
            const saved = await api.shows.saveShowCopy(source.id, name, target, baseShow);
            if (!target.instance) setShows(await api.shows.shows());
            return saved;
        },
        exportMvrFile: async (name, target) => {
            const source = model.bootstrap?.active_show;
            if (!source) throw new Error("Open a show before exporting MVR");
            return api.shows.exportMvrFile(source.id, name, target);
        },
        networkShows: () => api.shows.networkShows(),
        importRemoteShow: async (instance, id, revision, open) => {
            try {
                const show = await api.shows.importRemoteShow(instance, id, revision, open);
                if (open) await refresh();
                setShows(await api.shows.shows()); setError(null); return show;
            } catch(reason) {setError(formatErrorDetails(reason)); return null;}
        },
        prepareShowRevision: async (id, revision) => {
            try { const show = await api.shows.prepareRevision(id, revision); setShows(await api.shows.shows()); setError(null); return show; }
            catch(reason) {setError(formatErrorDetails(reason)); return null;}
        },
        prepareShowFile: async (root, path, name) => {
            try {
                const entry = root === "shows" ? shows.find(show => show.path.replaceAll("\\", "/").endsWith(`/${path}`)) : undefined;
                if (entry) return entry;
                const blob = await api.files.fileContent(root, path);
                const bytes = new Uint8Array(await blob.arrayBuffer());
                let binary = ""; for (const byte of bytes) binary += String.fromCharCode(byte);
                const imported = await api.shows.createShow(name.replace(/\.show$/i, ""), btoa(binary), false);
                setShows(await api.shows.shows()); setError(null); return imported;
            } catch(reason) {setError(formatErrorDetails(reason)); return null;}
        },
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
				setError(formatErrorDetails(reason));
			}
		},
		/**
		 * Only editors holding a document: a peer with nothing open is discoverable but has
		 * nothing to offer, and offering it would be offering a failure.
		 */
		discoveredVisualizers: async () => {
			try {
				const found = await api.discovery.peers();
				return found.peers.filter(
					(peer) => peer.role === "editor" && peer.show !== null,
				);
			} catch {
				// Discovery is a convenience: a desk that cannot look simply offers nothing.
				return [];
			}
		},
		loadFromVisualizer: async (instance) => {
			try {
				await whileLoadingShow(model, "Loading show from visualizer…", async () => {
					await api.shows.importFromVisualizer(instance);
					await refresh();
				});
				setShows(await api.shows.shows());
				setError(null);
				return true;
			} catch (reason) {
				setError(formatErrorDetails(reason));
				return false;
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
                return true;
			} catch (reason) {
				setError(formatErrorDetails(reason));
                return false;
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
				setError(formatErrorDetails(reason));
				return false;
			}
		},
		openShowFile: async (rootId, path, name) => {
			try {
				const showName = name.replace(/\.show$/i, "");
				await whileLoadingShow(model, `Loading show ${showName}…`, async () => {
                    let entry = rootId === "shows"
                        ? shows.find(show => show.path.replaceAll("\\", "/").endsWith(`/${path}`) || show.path === path)
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
				setError(formatErrorDetails(reason));
				return false;
			}
		},
	};
}
