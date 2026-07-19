import { useEffect, useMemo, useState } from "react";
import { useServer } from "../../api/ServerContext";
import type {
	AttributeValue,
	Cue,
	VisualizationSnapshot,
} from "../../api/types";
import { useDesktopBridge } from "../../platform/desktop";
import {
	cueVisualization,
	migrateStagePosition,
	renderStageThumbnail,
} from "../stage3dScene";

function useStageFixtures() {
	const server = useServer();
	return useMemo(
		() =>
			(server.patch?.fixtures ?? []).flatMap((fixture, fixtureIndex) =>
				[
					{
						id: fixture.fixture_id,
						location: fixture.location,
						rotation: fixture.rotation,
					},
					...(fixture.multipatch ?? []),
				].map((instance, instanceIndex) => {
					const index = fixtureIndex * 16 + instanceIndex;
					const located =
						instance.location &&
						(instance.location.x || instance.location.y || instance.location.z)
							? {
									x: instance.location.x / 1000,
									y: instance.location.y / 1000,
									z: instance.location.z / 1000,
									rotationX: instance.rotation?.x ?? 0,
									rotationY: instance.rotation?.y ?? 0,
									rotationZ: instance.rotation?.z ?? 0,
								}
							: null;
					return {
						fixture,
						instanceId: instance.id,
						index,
						position:
							server.stageLayout?.body.positions3d?.[instance.id] ??
							located ??
							migrateStagePosition(
								instanceIndex
									? undefined
									: server.stageLayout?.body.positions[fixture.fixture_id],
								index,
							),
					};
				}),
			),
		[server.patch, server.stageLayout],
	);
}

function cueChanges(cue: Cue, groups: ReturnType<typeof useServer>["groups"]) {
	const changes = [...(cue.changes ?? [])] as Array<{
		fixture_id: string;
		attribute: string;
		value: AttributeValue | null;
	}>;
	for (const groupChange of cue.group_changes ?? []) {
		const group = groups.find(
			(candidate) => candidate.id === groupChange.group_id,
		);
		for (const fixture_id of group?.body.fixtures ?? []) {
			changes.push({
				fixture_id,
				attribute: groupChange.attribute,
				value: groupChange.value,
			});
		}
	}
	return changes;
}

export function useCueThumbnails(cues: Cue[]) {
	const server = useServer();
	const tauri = useDesktopBridge().available;
	const stageFixtures = useStageFixtures();
	const [thumbnails, setThumbnails] = useState<Record<number, string>>({});
	useEffect(() => {
		if (!tauri || !cues.length || !stageFixtures.length) return;
		let cancelled = false;
		void server
			.readVisualization()
			.then((live) => {
				if (cancelled) return;
				let state: VisualizationSnapshot = { ...live, values: [] };
				const next: Record<number, string> = {};
				for (let index = 0; index < cues.length; index++) {
					state = cueVisualization(
						state,
						cueChanges(cues[index], server.groups),
					);
					next[index] = renderStageThumbnail(stageFixtures, state);
				}
				if (!cancelled) setThumbnails(next);
			})
			.catch(() => undefined);
		return () => {
			cancelled = true;
		};
	}, [tauri, cues, stageFixtures, server.groups, server.readVisualization]);
	return thumbnails;
}
