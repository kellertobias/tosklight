import { formatErrorDetails } from "@tosklight/ui";
import { ApiRequestError } from "../../api/ApiRequestError";
import type { VersionedObject } from "../../api/types";
import type { ServerCapabilities } from "./capabilityContracts";
import { DESK_LAYOUT_ID, type StoredDeskLayout } from "./contracts";
import type { ServerController } from "./model";

export function createLayoutActions(
	model: ServerController,
): Pick<ServerCapabilities, "saveDeskLayout"> {
	const {
		api,
		setError,
		bootstrap,
		session,
		deskLayout,
		setDeskLayout,
		showObjectsStore,
	} = model;
	return {
		saveDeskLayout: async (layout) => {
			const showId = bootstrap?.active_show?.id;
			const authority = showObjectsStore.getSnapshot();
			const current = () => {
				const snapshot = showObjectsStore.getSnapshot();
				return (
					snapshot.showId === showId &&
					snapshot.authorityGeneration === authority.authorityGeneration
				);
			};
			try {
				if (!showId || !session)
					throw new Error("Open a show before saving a Desktop layout");
				if (!current())
					throw new Error("Current Show layout authority is unavailable");
				// A layout this show already holds keeps its own id, so a show written before the
				// collapse is updated in place rather than left beside a second one.
				const layoutId = deskLayout?.id ?? DESK_LAYOUT_ID;
				const revision = deskLayout?.revision ?? 0;
				let outcome: Awaited<
					ReturnType<typeof api.showObjects.updateUserLayout>
				>;
				try {
					outcome = await api.showObjects.updateUserLayout(
						showId,
						layoutId,
						layout,
						revision,
					);
				} catch (reason) {
					if (!(reason instanceof ApiRequestError) || reason.status !== 409)
						throw reason;
					if (!current()) return;
					const loaded = await api.showObjects.objectOrNull<StoredDeskLayout>(
						showId,
						"user_layout",
						layoutId,
					);
					if (!current()) return;
					outcome = await api.showObjects.updateUserLayout(
						showId,
						layoutId,
						layout,
						loaded?.revision ?? 0,
					);
				}
				if (!current()) return;
				if (
					outcome.show_id !== showId ||
					outcome.object.kind !== "user_layout" ||
					outcome.object.id !== layoutId
				)
					throw new Error(
						"Desktop layout save returned mismatched Show or object authority",
					);
				if (
					!showObjectsStore.installShowRevision(
						showId,
						outcome.show_revision,
						authority.authorityGeneration,
					)
				)
					return;
				const saved = outcome.object as VersionedObject<StoredDeskLayout>;
				setDeskLayout((loaded) =>
					!current() ||
					(loaded?.id === layoutId && loaded.revision > saved.revision)
						? loaded
						: saved,
				);
				setError(null);
			} catch (reason) {
				if (showId && authority.showId === showId && !current()) return;
				setError(formatErrorDetails(reason));
			}
		},
	};
}
