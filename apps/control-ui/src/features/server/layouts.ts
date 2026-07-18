import type { StoredDeskLayout, StoredStageLayout } from "./contracts";
import type { ServerController } from "./model";
import type { ServerContextValue } from "./ServerContextValue";

export function createLayoutActions(
	model: ServerController,
): Pick<ServerContextValue, "saveDeskLayout" | "saveStageLayout"> {
	const {
		client,
		setError,
		bootstrap,
		session,
		deskLayout,
		setDeskLayout,
		stageLayout,
		setStageLayout,
	} = model;
	return {
		saveDeskLayout: async (layout) => {
			try {
				if (!bootstrap?.active_show || !session)
					throw new Error("Open a show before saving a Desktop layout");
				const revision = deskLayout?.revision ?? 0;
				await client.putObject(
					bootstrap.active_show.id,
					"user_layout",
					session.user.id,
					layout,
					revision,
				);
				const layouts = await client.objects<StoredDeskLayout>(
					bootstrap.active_show.id,
					"user_layout",
				);
				setDeskLayout(
					layouts.find((item) => item.id === session.user.id) ?? null,
				);
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		saveStageLayout: async (layout) => {
			try {
				if (!bootstrap?.active_show)
					throw new Error("Open a show before saving stage positions");
				await client.putObject(
					bootstrap.active_show.id,
					"stage_layout",
					"main",
					layout,
					stageLayout?.revision ?? 0,
				);
				const layouts = await client.objects<StoredStageLayout>(
					bootstrap.active_show.id,
					"stage_layout",
				);
				setStageLayout(layouts.find((item) => item.id === "main") ?? null);
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
	};
}
