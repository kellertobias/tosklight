import type { ServerController } from "./model";
import type { ServerCapabilities } from "./capabilityContracts";

export function createMvrActions(
	model: ServerController,
): Pick<
	ServerCapabilities,
	"previewMvr" | "applyMvr" | "previewMvrExport" | "downloadMvr"
> {
	const { api, setError, setShows, refresh } = model;
	return {
		previewMvr: (file, showId) => api.shows.previewMvr(file, showId),
		applyMvr: async (token, input) => {
			try {
				const result = await api.shows.applyMvr(token, input);
				await refresh();
				setShows(await api.shows.shows());
				setError(null);
				return result;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				throw reason;
			}
		},
		previewMvrExport: (showId) => api.shows.mvrExportPreview(showId),
		downloadMvr: async (show) => {
			try {
				const blob = await api.shows.downloadMvr(show.id);
				const url = URL.createObjectURL(blob);
				const anchor = document.createElement("a");
				anchor.href = url;
				anchor.download = `${show.name}.mvr`;
				anchor.click();
				URL.revokeObjectURL(url);
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
	};
}
