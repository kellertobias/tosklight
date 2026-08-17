import type { ServerCapabilities } from "./capabilityContracts";
import type { ServerController } from "./model";

export function createMediaActions(
	model: ServerController,
): Pick<
	ServerCapabilities,
	| "refreshMediaPreview"
	| "refreshMediaThumbnails"
	| "inspectMediaServer"
	| "nativeMedia"
	| "updateNativeMediaText"
	| "applyMediaLibrarySelection"
	| "mediaThumbnail"
	| "discoverMediaServers"
	| "updateDiscoveredMediaAddress"
> {
	const {
		api,
		setError,
		setMediaServers,
		setMediaPreviewUrls,
		mediaPreviewUrlsRef,
	} = model;
	const recordStatus = (
		fixtureId: string,
		online: boolean,
		lastError: string | null,
	) => {
		setMediaServers((current) => {
			let changed = false;
			const next = current.map((fixture) => {
				if (
					fixture.fixture_id !== fixtureId ||
					(fixture.status.online === online &&
						fixture.status.last_error === lastError)
				)
					return fixture;
				changed = true;
				return {
					...fixture,
					status: {
						online,
						last_success: online
							? new Date().toISOString()
							: fixture.status.last_success,
						last_error: lastError,
					},
				};
			});
			return changed ? next : current;
		});
	};
	return {
		discoverMediaServers: () => api.mediaOutput.discoverMediaServers(),
		updateDiscoveredMediaAddress: (input) =>
			api.mediaOutput.updateDiscoveredMediaAddress(input),
		inspectMediaServer: async (fixtureId) => {
			try {
				const inspection = await api.mediaOutput.inspectMediaServer(fixtureId);
				recordStatus(fixtureId, true, null);
				return inspection;
			} catch (reason) {
				recordStatus(
					fixtureId,
					false,
					reason instanceof Error ? reason.message : String(reason),
				);
				throw reason;
			}
		},
		nativeMedia: (fixtureId) => api.mediaOutput.nativeMedia(fixtureId),
		updateNativeMediaText: (fixtureId, folder, file, text) =>
			api.mediaOutput.updateNativeMediaText(fixtureId, folder, file, text),
		applyMediaLibrarySelection: (fixtureId, input) =>
			api.mediaOutput.applyMediaLibrarySelection(fixtureId, input),
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
						[fixtureId]: url,
					};
					mediaPreviewUrlsRef.current = next;
					return next;
				});
				recordStatus(fixtureId, true, null);
				setError(null);
				return true;
			} catch (reason) {
				const message =
					reason instanceof Error ? reason.message : String(reason);
				recordStatus(fixtureId, false, message);
				setError(message);
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
				recordStatus(fixtureId, true, null);
				setError(null);
			} catch (reason) {
				const message =
					reason instanceof Error ? reason.message : String(reason);
				recordStatus(fixtureId, false, message);
				setError(message);
			}
		},
	};
}
