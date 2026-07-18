import type { ServerController } from "./model";
import type { ServerContextValue } from "./ServerContextValue";

export function createShowRevisionActions(
	model: ServerController,
): Pick<
	ServerContextValue,
	| "listShowRevisions"
	| "saveShowRevision"
	| "openShowRevision"
	| "rollbackShow"
	| "downloadShow"
> {
	const { client, setError, bootstrap, refresh } = model;
	return {
		listShowRevisions: async (id) => {
			try {
				const revisions = await client.showRevisions(id);
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
				const revision = await client.saveShowRevision(
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
				await client.openShowRevision(id, revision);
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
				await client.rollbackShow();
				await refresh();
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		downloadShow: async (show) => {
			try {
				const blob = await client.downloadShow(show.id);
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
