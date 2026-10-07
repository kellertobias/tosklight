import { formatErrorDetails } from "@tosklight/ui";
import type { ServerController } from "./model";
import type { ServerCapabilities } from "./capabilityContracts";

export function createMvrActions(
	model: ServerController,
): Pick<
	ServerCapabilities,
	"previewMvr" | "applyMvr"
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
				setError(formatErrorDetails(reason));
				throw reason;
			}
		},
	};
}
