import type { ServerController } from "./model";
import type { ServerContextValue } from "./ServerContextValue";

export function createPlaybackConfigurationActions(
	model: ServerController,
): Pick<
	ServerContextValue,
	| "savePlaybackPage"
	| "savePlaybackDefinition"
	| "savePlaybackSlot"
	| "clearPlaybackSlot"
> {
	const { client, setError, bootstrap, playbacks, setPlaybacks, refresh } =
		model;
	return {
		savePlaybackPage: async (page) => {
			if (!bootstrap?.active_show) return false;
			try {
				const pages = await client.objects<
					import("../../api/types").PlaybackPage
				>(bootstrap.active_show.id, "playback_page");
				const existing = pages.find((item) => item.body.number === page.number);
				if (!existing) {
					for (const loadedPage of playbacks?.pages ?? []) {
						if (
							loadedPage.number === page.number ||
							pages.some((item) => item.body.number === loadedPage.number)
						)
							continue;
						await client.putObject(
							bootstrap.active_show.id,
							"playback_page",
							String(loadedPage.number),
							loadedPage,
							0,
						);
					}
				}
				await client.putObject(
					bootstrap.active_show.id,
					"playback_page",
					String(page.number),
					page,
					existing?.revision ?? 0,
				);
				setPlaybacks(await client.playbacks());
				setError(null);
				return true;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				return false;
			}
		},
		savePlaybackDefinition: async (playback) => {
			if (!bootstrap?.active_show) return;
			try {
				const objects = await client.objects<
					import("../../api/types").PlaybackDefinition
				>(bootstrap.active_show.id, "playback");
				const existing = objects.find(
					(item) => item.body.number === playback.number,
				);
				await client.putObject(
					bootstrap.active_show.id,
					"playback",
					String(playback.number),
					playback,
					existing?.revision ?? 0,
				);
				await refresh();
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		savePlaybackSlot: async (page, slot, playback) => {
			if (!bootstrap?.active_show) return false;
			try {
				const [pages, definitions] = await Promise.all([
					client.objects<import("../../api/types").PlaybackPage>(
						bootstrap.active_show.id,
						"playback_page",
					),
					client.objects<import("../../api/types").PlaybackDefinition>(
						bootstrap.active_show.id,
						"playback",
					),
				]);
				const pageObject = pages.find((item) => item.body.number === page);
				const mappedNumber = pageObject?.body.slots[String(slot)];
				const playbackObject =
					mappedNumber == null
						? undefined
						: definitions.find((item) => item.body.number === mappedNumber);
				await client.savePlaybackSlot(
					page,
					slot,
					playback,
					playbackObject?.revision ?? 0,
					pageObject?.revision ?? 0,
				);
				setPlaybacks(await client.playbacks());
				setError(null);
				return true;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				return false;
			}
		},
		clearPlaybackSlot: async (page, slot) => {
			if (!bootstrap?.active_show) return false;
			try {
				const [pages, definitions] = await Promise.all([
					client.objects<import("../../api/types").PlaybackPage>(
						bootstrap.active_show.id,
						"playback_page",
					),
					client.objects<import("../../api/types").PlaybackDefinition>(
						bootstrap.active_show.id,
						"playback",
					),
				]);
				const pageObject = pages.find((item) => item.body.number === page);
				const mappedNumber = pageObject?.body.slots[String(slot)];
				if (!pageObject || mappedNumber == null) return true;
				const playbackObject = definitions.find(
					(item) => item.body.number === mappedNumber,
				);
				if (!playbackObject) return false;
				await client.clearPlaybackSlot(
					page,
					slot,
					playbackObject.revision,
					pageObject.revision,
				);
				setPlaybacks(await client.playbacks());
				setError(null);
				return true;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				return false;
			}
		},
	};
}
