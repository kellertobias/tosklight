import { PoolGrid, type PoolSlotViewModel } from "@tosklight/ui/pools";
import { useRef } from "react";
import type { CommandLineSurface } from "../../components/control/commandLine/useCommandLineSurface";
import { requestUpdateTarget } from "../../components/control/updateWorkflow";
import { WindowScrollArea } from "@tosklight/ui/window-kit";
import {
	captureGroupRecordingTarget,
	emptyGroupRecordingTarget,
	type GroupRecordingTarget,
} from "../../features/groupRecording/target";
import { useGroupSelectionActions } from "../../features/groupSelection/useGroupSelectionActions";
import { useApp } from "../../state/AppContext";
import { GroupCard } from "./GroupCard";
import type { FixtureMetadata, Group } from "./model";
import {
	poolSurfaceKey,
	usePoolPresentationConfiguration,
} from "../../features/poolPresentation/poolPresentation";
import { useActiveShowId } from "../../features/deskSnapshot/DeskSnapshotState";

export function GroupPoolGrid({
	active = true,
	cards,
	capabilities,
	knownFixtureIds,
	command,
	onOpenContext,
	onOpenProperties,
	onOpenRecord,
	recordGroup,
	runCommand,
	paneId,
}: Pick<FixtureMetadata, "capabilities" | "knownFixtureIds"> & {
	active?: boolean;
	cards: (Group | null)[];
	command: CommandLineSurface;
	onOpenContext: (id: string) => void;
	onOpenProperties: (id: string) => void;
	onOpenRecord: (target: GroupRecordingTarget) => void;
	recordGroup: (target: GroupRecordingTarget) => Promise<unknown>;
	runCommand: (command: string) => Promise<unknown>;
	paneId?: string;
}) {
	const groupSelection = useGroupSelectionActions(active);
	const { state, dispatch } = useApp();
	const poolPresentation = usePoolPresentationConfiguration();
	const showId = useActiveShowId() ?? "unresolved";
	const surfaceKey = poolSurfaceKey(showId, "group", paneId);
	const hold = useRef<number | null>(null);
	const cancelHold = () => {
		if (hold.current) window.clearTimeout(hold.current);
		hold.current = null;
	};
	const selectCard = (group: Group | null, index: number) => {
		const id = group?.id ?? String(index + 1);
		if (state.updateArmed) {
			requestUpdateTarget({ family: { type: "group" }, object_id: id });
			return;
		}
		if (group && /^SET\b/i.test(command.read().text.trim())) {
			onOpenProperties(group.id);
			void command.reset();
			return;
		}
		if (group && !state.storeArmed) {
			void groupSelection.selectLive(group);
			return;
		}
		if (!state.storeArmed) return;
		if (group?.body.fixtures.length) {
			onOpenRecord(captureGroupRecordingTarget(group));
			return;
		}
		void recordGroup(
			group
				? captureGroupRecordingTarget(group)
				: emptyGroupRecordingTarget(id),
		).finally(() => dispatch({ type: "SET_STORE_ARMED", value: false }));
	};
	const slots: PoolSlotViewModel<string>[] = cards.flatMap((group, index) =>
		group
			? [
					{
						id: group.id,
						position: index,
						card: {
							number: index + 1,
							primary: group.body.name ?? `Group ${index + 1}`,
						},
					},
				]
			: [],
	);

	return (
		<WindowScrollArea>
			<PoolGrid
				slots={slots}
				slotCount={cards.length}
				emptySlot={(index) => ({
					id: String(index + 1),
					position: index,
					card: { number: index + 1, primary: "Empty", states: ["empty"] },
				})}
				renderSlot={(_, index) => {
					const group = cards[index] ?? null;
					return (
						<GroupCard
							group={group}
							index={index}
							poolSlotId={String(group?.id ?? index + 1)}
							knownFixtureIds={knownFixtureIds}
							capabilities={capabilities}
							selected={command.selectedGroupId === group?.id}
							storeArmed={state.storeArmed}
							updateArmed={state.updateArmed}
							poolPresentation={poolPresentation}
							showId={showId}
							surfaceKey={surfaceKey}
							beginHold={() => {
								if (group && !state.updateArmed) {
									hold.current = window.setTimeout(
										() => onOpenContext(group.id),
										600,
									);
								}
							}}
							cancelHold={cancelHold}
							openContext={() => group && onOpenContext(group.id)}
							dereference={() => group && void runCommand(`DEGRP ${group.id}`)}
							select={() => selectCard(group, index)}
						/>
					);
				}}
			/>
		</WindowScrollArea>
	);
}
