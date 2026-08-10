import type { ServerController } from "./model";
import type { ServerCapabilities } from "./capabilityContracts";

export function createMediaActions(
	model: ServerController,
): Pick<
	ServerCapabilities,
	| "refreshMediaPreview"
	| "refreshMediaThumbnails"
	| "inspectMediaServer"
	| "mediaThumbnail"
> {
	const {
		api,
		setError,
		mediaServers,
		setMediaServers,
		setMediaPreviewUrls,
		mediaPreviewUrlsRef,
	} = model;
	return {
		inspectMediaServer: (fixtureId) =>
			api.mediaOutput.inspectMediaServer(fixtureId),
		mediaThumbnail: (fixtureId, folder, element) =>
			api.mediaOutput.mediaThumbnail(fixtureId, folder, element),
		refreshMediaPreview: async (fixtureId, source = 0) => {
			try {
				await api.mediaOutput.refreshMediaPreview(fixtureId, source);
				const blob = await api.mediaOutput.mediaPreview(fixtureId, source);
				const url = URL.createObjectURL(blob);
				setMediaPreviewUrls((current) => {
					const sourceKey = `${fixtureId}:${source}`;
					const previous = current[sourceKey];
					if (previous) URL.revokeObjectURL(previous);
					const next = {
						...current,
						[sourceKey]: url,
					};
					if (source === 0) next[fixtureId] = url;
					mediaPreviewUrlsRef.current = next;
					return next;
				});
				setMediaServers((await api.mediaOutput.mediaServers()).fixtures);
				setError(null);
				return true;
			} catch (reason) {
				setMediaServers(
					(
						await api.mediaOutput
							.mediaServers()
							.catch(() => ({ fixtures: mediaServers }))
					).fixtures,
				);
				setError(reason instanceof Error ? reason.message : String(reason));
				return false;
			}
		},
		refreshMediaThumbnails: async (fixtureId, folder, elements) => {
			try {
				await api.mediaOutput.refreshMediaThumbnails(
					fixtureId,
					folder,
					elements,
				);
				setMediaServers((await api.mediaOutput.mediaServers()).fixtures);
				setError(null);
			} catch (reason) {
				setMediaServers(
					(
						await api.mediaOutput
							.mediaServers()
							.catch(() => ({ fixtures: mediaServers }))
					).fixtures,
				);
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
	};
}
