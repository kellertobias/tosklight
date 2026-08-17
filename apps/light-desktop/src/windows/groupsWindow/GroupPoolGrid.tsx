import {
	DEFAULT_POOL_CARD_MINIMUM_WIDTH,
	PoolGrid,
	type PoolSlotViewModel,
} from "@tosklight/ui/pools";
import { WindowScrollArea } from "@tosklight/ui/window-kit";
import { type MutableRefObject, useRef } from "react";
import type { CommandLineSurface } from "../../components/control/commandLine/useCommandLineSurface";
import { requestUpdateTarget } from "../../components/control/updateWorkflow";
import {
	type PoolMutationOperation,
	poolMutationTarget,
	poolObjectMutationCommand,
} from "../../features/controlSurfaceInteraction/poolCommandTarget";
import { useSetInteraction } from "../../features/controlSurfaceInteraction/SetInteractionProvider";
import { useActiveShowId } from "../../features/deskSnapshot/DeskSnapshotState";
import {
	captureGroupRecordingTarget,
	emptyGroupRecordingTarget,
	type GroupRecordingTarget,
} from "../../features/groupRecording/target";
import { useGroupSelectionActions } from "../../features/groupSelection/useGroupSelectionActions";
import {
	poolSurfaceKey,
	usePoolPresentationConfiguration,
} from "../../features/poolPresentation/poolPresentation";
import { useApp } from "../../state/AppContext";
import { GroupCard } from "./GroupCard";
import type { FixtureMetadata, Group } from "./model";

interface GroupPoolCardSlotProps
	extends Pick<FixtureMetadata, "capabilities" | "knownFixtureIds"> {
	group: Group | null;
	index: number;
	selected: boolean;
	storeArmed: boolean;
	updateArmed: boolean;
	setTarget: boolean;
	mutationOperation: PoolMutationOperation | null;
	poolPresentation: ReturnType<typeof usePoolPresentationConfiguration>;
	showId: string;
	surfaceKey: string;
	setInteraction: ReturnType<typeof useSetInteraction>;
	select(): void;
	chooseSetSource(group: Group): void;
	dereference(group: Group): void;
	onOpenSettings(id: string): void;
	hold: MutableRefObject<number | null>;
	held: MutableRefObject<boolean>;
}

function GroupPoolCardSlot({
	group,
	index,
	selected,
	storeArmed,
	updateArmed,
	setTarget,
	mutationOperation,
	poolPresentation,
	showId,
	surfaceKey,
	setInteraction,
	select,
	chooseSetSource,
	dereference,
	onOpenSettings,
	knownFixtureIds,
	capabilities,
	hold,
	held,
}: GroupPoolCardSlotProps) {
	const cancelHold = () => {
		if (hold.current) window.clearTimeout(hold.current);
		hold.current = null;
	};
	const openSettings = (source: "touch" | "context_menu") => {
		if (!group) return;
		const scope = setInteraction?.state?.scope;
		if (!scope) return onOpenSettings(group.id);
		void setInteraction.direct({
			type: "open_group_settings",
			source,
			scope,
			group: { objectId: group.id, objectRevision: group.revision },
		});
	};
	return (
		<GroupCard
			group={group}
			index={index}
			poolSlotId={String(group?.id ?? index + 1)}
			knownFixtureIds={knownFixtureIds}
			capabilities={capabilities}
			selected={selected}
			storeArmed={storeArmed}
			updateArmed={updateArmed}
			setTarget={setTarget}
			mutationOperation={mutationOperation}
			poolPresentation={poolPresentation}
			showId={showId}
			surfaceKey={surfaceKey}
			beginHold={() => {
				if (group && !updateArmed) {
					held.current = false;
					hold.current = window.setTimeout(() => {
						held.current = true;
						openSettings("touch");
					}, 600);
				}
			}}
			cancelHold={cancelHold}
			consumeHold={() => {
				const consumed = held.current;
				held.current = false;
				return consumed;
			}}
			openSettings={() => openSettings("context_menu")}
			dereference={() => group && dereference(group)}
			select={select}
		/>
	);
}

function groupPoolSlots(cards: (Group | null)[]): PoolSlotViewModel<string>[] {
	return cards.flatMap((group, index) =>
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
}

function groupSetSourceChooser(
	setInteraction: ReturnType<typeof useSetInteraction>,
	command: CommandLineSurface,
	dispatch: ReturnType<typeof useApp>["dispatch"],
) {
	return async (group: Group) => {
		if (!setInteraction?.state) {
			await command.replace(`ASSIGN GROUP ${group.id}`, false);
			dispatch({ type: "SET_PLAYBACK_SET_ARMED", value: false });
			return;
		}
		if (setInteraction.state.phase === "idle") {
			if (!(await setInteraction.arm("context_menu"))) return;
		} else if (setInteraction.state.phase !== "set_armed") return;
		await setInteraction.chooseGroup(
			{ objectId: group.id, objectRevision: group.revision },
			"context_menu",
		);
	};
}

function groupDereferencer(
	setInteraction: ReturnType<typeof useSetInteraction>,
	groupSelection: ReturnType<typeof useGroupSelectionActions>,
) {
	return (group: Group) => {
		const scope = setInteraction?.state?.scope;
		if (scope)
			void setInteraction.direct({
				type: "select_group_frozen",
				source: "touch",
				scope,
				group: { objectId: group.id, objectRevision: group.revision },
			});
		void groupSelection.selectFrozen(group);
	};
}

export function GroupPoolGrid({
	active = true,
	cards,
	capabilities,
	knownFixtureIds,
	command,
	onOpenSettings,
	onOpenRecord,
	recordGroup,
	paneId,
	columns,
}: Pick<FixtureMetadata, "capabilities" | "knownFixtureIds"> & {
	active?: boolean;
	cards: (Group | null)[];
	command: CommandLineSurface;
	onOpenSettings: (id: string) => void;
	onOpenRecord: (target: GroupRecordingTarget) => void;
	recordGroup: (target: GroupRecordingTarget) => Promise<unknown>;
	paneId?: string;
	columns?: number;
}) {
	const groupSelection = useGroupSelectionActions(active);
	const setInteraction = useSetInteraction();
	const { state, dispatch } = useApp();
	const poolPresentation = usePoolPresentationConfiguration();
	const showId = useActiveShowId() ?? "unresolved";
	const surfaceKey = poolSurfaceKey(showId, "group", paneId);
	const chooseSetSource = groupSetSourceChooser(
		setInteraction,
		command,
		dispatch,
	);
	const dereference = groupDereferencer(setInteraction, groupSelection);
	const hold = useRef<number | null>(null);
	const held = useRef(false);
	const mutationTarget = poolMutationTarget(command.text);
	const setTargetArmed =
		setInteraction?.state?.phase === "set_armed" ||
		/^SET$/iu.test(command.text.trim());
	const selectCard = (group: Group | null, index: number) => {
		const id = group?.id ?? String(index + 1);
		if (state.updateArmed) {
			requestUpdateTarget({ family: { type: "group" }, object_id: id });
			return;
		}
		const mutation = poolObjectMutationCommand(
			mutationTarget,
			"GROUP",
			id,
			group !== null,
		);
		if (mutation) {
			if (mutation.kind === "execute") void command.execute(mutation.command);
			else void command.replace(mutation.command, false);
			return;
		}
		if (group && setInteraction?.state?.phase === "set_armed") {
			void setInteraction.chooseGroup(
				{ objectId: group.id, objectRevision: group.revision },
				"touch",
			);
			return;
		}
		if (!setInteraction) {
			const commandText = command.read().text.trim();
			if (
				group &&
				state.controlMode === "playbacks" &&
				(state.playbackSetArmed || /^ASSIGN$/i.test(commandText))
			) {
				void command.replace(`ASSIGN GROUP ${group.id}`, false);
				dispatch({ type: "SET_PLAYBACK_SET_ARMED", value: false });
				return;
			}
			if (group && /^SET\b/i.test(commandText)) {
				onOpenSettings(group.id);
				void command.reset();
				return;
			}
		}
		if (group && !state.storeArmed) {
			const scope = setInteraction?.state?.scope;
			if (scope)
				void setInteraction.direct({
					type: "select_group_live",
					source: "touch",
					scope,
					group: { objectId: group.id, objectRevision: group.revision },
				});
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
	const slots = groupPoolSlots(cards);
	return (
		<WindowScrollArea>
			<PoolGrid
				columns={columns}
				minimumCardWidth={DEFAULT_POOL_CARD_MINIMUM_WIDTH}
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
						<GroupPoolCardSlot
							group={group}
							index={index}
							selected={command.selectedGroupId === group?.id}
							storeArmed={state.storeArmed}
							updateArmed={state.updateArmed}
							setTarget={Boolean(group && setTargetArmed)}
							mutationOperation={
								poolObjectMutationCommand(
									mutationTarget,
									"GROUP",
									group?.id ?? index + 1,
									group !== null,
								)
									? (mutationTarget?.operation ?? null)
									: null
							}
							poolPresentation={poolPresentation}
							showId={showId}
							surfaceKey={surfaceKey}
							setInteraction={setInteraction}
							select={() => selectCard(group, index)}
							chooseSetSource={(target) => void chooseSetSource(target)}
							dereference={dereference}
							onOpenSettings={onOpenSettings}
							knownFixtureIds={knownFixtureIds}
							capabilities={capabilities}
							hold={hold}
							held={held}
						/>
					);
				}}
			/>
		</WindowScrollArea>
	);
}
