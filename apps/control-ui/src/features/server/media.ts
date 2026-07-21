import type { ServerController } from "./model";
import type { ServerContextValue } from "./ServerContextValue";

export function createMediaActions(
	model: ServerController,
): Pick<ServerContextValue, "refreshMediaPreview" | "refreshMediaThumbnails"> {
	const {
		client,
		setError,
		mediaServers,
		setMediaServers,
		setMediaPreviewUrls,
		mediaPreviewUrlsRef,
	} = model;
	return {
		refreshMediaPreview: async (fixtureId, source = 0) => {
			try {
				await client.refreshMediaPreview(fixtureId, source);
				const blob = await client.mediaPreview(fixtureId, source);
				const url = URL.createObjectURL(blob);
				setMediaPreviewUrls((current) => {
					const previous = current[fixtureId];
					if (previous) URL.revokeObjectURL(previous);
					const next = { ...current, [fixtureId]: url };
					mediaPreviewUrlsRef.current = next;
					return next;
				});
				setMediaServers((await client.mediaServers()).fixtures);
				setError(null);
				return true;
			} catch (reason) {
				setMediaServers(
					(
						await client
							.mediaServers()
							.catch(() => ({ fixtures: mediaServers }))
					).fixtures,
				);
				setError(reason instanceof Error ? reason.message : String(reason));
				return false;
			}
		},
		refreshMediaThumbnails: async (fixtureId, elements) => {
			try {
				await client.refreshMediaThumbnails(fixtureId, elements);
				setMediaServers((await client.mediaServers()).fixtures);
				setError(null);
			} catch (reason) {
				setMediaServers(
					(
						await client
							.mediaServers()
							.catch(() => ({ fixtures: mediaServers }))
					).fixtures,
				);
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
	};
}
