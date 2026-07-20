import type { ServerController } from "./model";
import type { ServerContextValue } from "./ServerContextValue";

export function createCueListActions(
	model: ServerController,
): Pick<ServerContextValue, "unassignPagePlayback"> {
	const { client, setError, bootstrap, refresh } = model;
	return {
		unassignPagePlayback: async (pageNumber, slot) => {
			if (!bootstrap?.active_show) return false;
			try {
				const pages = await client.objects<
					import("../../api/types").PlaybackPage
				>(bootstrap.active_show.id, "playback_page");
				const page = pages.find((item) => item.body.number === pageNumber);
				if (!page || page.body.slots[String(slot)] == null) return true;
				const playbackNumber = page.body.slots[String(slot)];
				await client
					.poolPlaybackAction(playbackNumber, "off")
					.catch(() => undefined);
				const slots = { ...page.body.slots };
				delete slots[String(slot)];
				await client.putObject(
					bootstrap.active_show.id,
					"playback_page",
					page.id,
					{ ...page.body, slots },
					page.revision,
				);
				await refresh();
				setError(null);
				return true;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				return false;
			}
		},
	};
}
