import { useEffect, useState } from "react";
import { GroupSettingsDialog } from "../../windows/groupsWindow/GroupSettingsDialog";
import { useActiveShowId } from "../deskSnapshot/DeskSnapshotState";
import {
	usePortableGroups,
	useShowObjectCollectionsReady,
} from "../showObjects/ShowObjectsState";
import { useShowObjectView } from "../showObjects/ShowObjectsView";
import { useControlSurfaceTarget } from "./useControlSurfaceTarget";

/** Fallback modal owner when the originating Group surface is not itself mounted. */
export function GroupSettingsIntentHost() {
	const showId = useActiveShowId();
	const groups = usePortableGroups(showId !== null);
	const groupsReady = useShowObjectCollectionsReady(["group"], showId !== null);
	useShowObjectView("group", showId !== null);
	const [objectId, setObjectId] = useState<string | null>(null);
	useEffect(() => setObjectId(null), [showId]);
	useControlSurfaceTarget({
		id: "group-settings-intent-host",
		priority: 10,
		accepts: (intent) =>
			groupsReady &&
			intent.type === "open_group_settings" &&
			groups.some(
				(group) =>
					group.id === intent.group.objectId &&
					group.revision === intent.group.objectRevision,
			),
		handle: (intent) => {
			if (intent.type === "open_group_settings")
				setObjectId(intent.group.objectId);
		},
	});
	const group = groups.find((candidate) => candidate.id === objectId);
	return group ? (
		<GroupSettingsDialog
			key={group.id}
			group={group}
			groups={groups}
			onClose={() => setObjectId(null)}
		/>
	) : null;
}
