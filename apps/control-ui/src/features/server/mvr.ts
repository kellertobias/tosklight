import type { ServerController } from "./model";
import type { ServerContextValue } from "./ServerContextValue";

export function createMvrActions(
	model: ServerController,
): Pick<
	ServerContextValue,
	"previewMvr" | "applyMvr" | "previewMvrExport" | "downloadMvr"
> {
	const { client, setError, setShows, refresh } = model;
	return {
		previewMvr: (file, showId) => client.previewMvr(file, showId),
		applyMvr: async (token, input) => {
			try {
				const result = await client.applyMvr(token, input);
				await refresh();
				setShows(await client.shows());
				setError(null);
				return result;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				throw reason;
			}
		},
		previewMvrExport: (showId) => client.mvrExportPreview(showId),
		downloadMvr: async (show) => {
			try {
				const blob = await client.downloadMvr(show.id);
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
