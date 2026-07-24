import type { ServerController } from "./model";
import type { ServerCapabilities } from "./capabilityContracts";

export function createShowRevisionActions(
	model: ServerController,
): Pick<
	ServerCapabilities,
	| "listShowRevisions"
	| "saveShowRevision"
	| "openShowRevision"
	| "rollbackShow"
	| "downloadShow"
> {
	const { api, setError, bootstrap, refresh } = model;
	return {
		listShowRevisions: async (id) => {
			try {
				const revisions = await api.shows.showRevisions(id);
				setError(null);
				return revisions;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				return [];
			}
		},
		saveShowRevision: async (name) => {
			try {
				if (!bootstrap?.active_show)
					throw new Error("Open a show before saving a named revision");
				const revision = await api.shows.saveShowRevision(
					bootstrap.active_show.id,
					name,
				);
				setError(null);
				return revision;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				return null;
			}
		},
		openShowRevision: async (id, revision) => {
			try {
				await api.shows.openShowRevision(id, revision);
				await refresh();
				setError(null);
				return true;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				return false;
			}
		},
		rollbackShow: async () => {
			try {
				await api.shows.rollbackShow();
				await refresh();
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		downloadShow: async (show) => {
			try {
				const blob = await api.shows.downloadShow(show.id);
				const url = URL.createObjectURL(blob);
				const anchor = document.createElement("a");
				anchor.href = url;
				anchor.download = `${show.name}.show`;
				anchor.click();
				URL.revokeObjectURL(url);
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
	};
}
