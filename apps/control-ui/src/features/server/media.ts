import type { ServerController } from "./model";
import type { ServerContextValue } from "./ServerContextValue";

export function createMediaActions(
	model: ServerController,
): Pick<
	ServerContextValue,
	"refreshMediaPreview" | "refreshMediaThumbnails" | "configureMediaServer"
> {
	const {
		client,
		setError,
		bootstrap,
		setPatch,
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
		configureMediaServer: async (fixtureId, ipAddress, port = 4811) => {
			try {
				if (!bootstrap?.active_show)
					throw new Error("Open a show before configuring media servers");
				const fixtures = await client.objects<
					import("../../api/types").PatchedFixture
				>(bootstrap.active_show.id, "patched_fixture");
				const object = fixtures.find(
					(candidate) => candidate.body.fixture_id === fixtureId,
				);
				if (!object) throw new Error("Patched fixture object was not found");
				const direct_control = ipAddress
					? { protocol: "citp" as const, ip_address: ipAddress, port }
					: null;
				await client.putObject(
					bootstrap.active_show.id,
					"patched_fixture",
					object.id,
					{ ...object.body, direct_control },
					object.revision,
				);
				setPatch(await client.patch());
				setMediaServers((await client.mediaServers()).fixtures);
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
	};
}
