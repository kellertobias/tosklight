import { Button } from "@tosklight/ui";
import { ButtonGrid } from "@tosklight/ui/window-kit";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { PoolPresentationConfiguration } from "../../api/types";
import { groups } from "../../data/mockData";
import {
	canonicalPoolMutationOperation,
	type PoolMutationOperation,
	poolMutationTarget,
	poolObjectMutationCommand,
} from "../../features/controlSurfaceInteraction/poolCommandTarget";
import { useSetInteraction } from "../../features/controlSurfaceInteraction/SetInteractionProvider";
import {
	useActiveShowId,
	useBootstrapReady,
} from "../../features/deskSnapshot/DeskSnapshotState";
import { useGroupRecording } from "../../features/groupRecording/GroupRecordingProvider";
import {
	captureGroupRecordingTarget,
	emptyGroupRecordingTarget,
	type GroupRecordingTarget,
} from "../../features/groupRecording/target";
import { useGroupSelectionActions } from "../../features/groupSelection/useGroupSelectionActions";
import {
	poolSurfaceKey,
	resolveConfiguredPoolPresentation,
	usePoolPresentationConfiguration,
} from "../../features/poolPresentation/poolPresentation";
import { usePortableGroups } from "../../features/showObjects/ShowObjectsState";
import { useShowObjectView } from "../../features/showObjects/ShowObjectsView";
import { useApp } from "../../state/AppContext";
import { useCommandLineSurface } from "../control/commandLine/useCommandLineSurface";
import { requestUpdateTarget } from "../control/updateWorkflow";
import { type RecordMode, RecordModeDialog } from "./RecordModeDialog";

const MIN_SHORTCUT_SIZE = 88;
const SHORTCUT_GAP = 2;

type ShortcutGroup = ReturnType<typeof usePortableGroups>[number];

export function groupShortcutCount(width: number) {
	return Math.max(
		1,
		Math.floor((width + SHORTCUT_GAP) / (MIN_SHORTCUT_SIZE + SHORTCUT_GAP)),
	);
}

function useGroupShortcutCount(active: boolean) {
	const gridRef = useRef<HTMLDivElement>(null);
	const [slotCount, setSlotCount] = useState(10);
	useLayoutEffect(() => {
		if (!active) return;
		const grid = gridRef.current;
		if (!grid) return;
		const measure = () => setSlotCount(groupShortcutCount(grid.clientWidth));
		measure();
		if (typeof ResizeObserver === "undefined") return;
		const observer = new ResizeObserver(measure);
		observer.observe(grid);
		return () => observer.disconnect();
	}, [active]);
	return { gridRef, slotCount };
}

function shortcutDescription(
	group: ShortcutGroup | null,
	storeArmed: boolean,
	updateArmed: boolean,
) {
	if (updateArmed) return "Touch to check Update eligibility";
	if (group)
		return group.body.fixtures.length
			? `${group.body.fixtures.length} fixtures`
			: "Group is empty";
	return storeArmed ? "Tap to record" : "Press Rec first";
}

function GroupShortcut({
	group,
	index,
	selected,
	storeArmed,
	updateArmed,
	setTarget,
	mutationOperation,
	onClick,
	onDoubleClick,
	onContextMenu,
	poolPresentation,
	showId,
	viewOnly,
}: {
	group: ShortcutGroup | null;
	index: number;
	selected: boolean;
	storeArmed: boolean;
	updateArmed: boolean;
	setTarget: boolean;
	mutationOperation: PoolMutationOperation | null;
	onClick: () => void;
	onDoubleClick: () => void;
	onContextMenu: () => void;
	poolPresentation: PoolPresentationConfiguration;
	showId: string;
	viewOnly: boolean;
}) {
	const clickTimer = useRef<number | null>(null);
	useEffect(
		() => () => {
			if (clickTimer.current !== null) window.clearTimeout(clickTimer.current);
		},
		[],
	);
	const presentation = resolveConfiguredPoolPresentation(poolPresentation, {
		showId,
		surfaceKey: poolSurfaceKey(showId, "group"),
		objectType: "group",
		itemColorKey: group?.id,
		itemColor: group?.body.color,
		states: [
			...(selected ? (["selected"] as const) : []),
			...(group ? [] : (["empty"] as const)),
			...(storeArmed ? (["record-target"] as const) : []),
			...(storeArmed ? (["store-target"] as const) : []),
			...(updateArmed ? (["update-target"] as const) : []),
			...(setTarget ? (["set-target"] as const) : []),
			...(mutationOperation ? ([`${mutationOperation}-target`] as const) : []),
		],
	});
	return (
		<Button
			className={`group-card pool-cell ${presentation.className}`}
			style={presentation.style}
			aria-pressed={selected}
			disabled={viewOnly}
			onClick={
				viewOnly
					? undefined
					: () => {
							if (clickTimer.current !== null)
								window.clearTimeout(clickTimer.current);
							clickTimer.current = window.setTimeout(() => {
								clickTimer.current = null;
								onClick();
							}, 240);
						}
			}
			onDoubleClick={
				viewOnly
					? undefined
					: () => {
							if (clickTimer.current !== null)
								window.clearTimeout(clickTimer.current);
							clickTimer.current = null;
							onDoubleClick();
						}
			}
			onContextMenu={(event) => {
				event.preventDefault();
				if (clickTimer.current !== null)
					window.clearTimeout(clickTimer.current);
				clickTimer.current = null;
				onContextMenu();
			}}
		>
			<span className="number">{index + 1}</span>
			<b>{group?.body.name ?? "Empty"}</b>
			<small>{shortcutDescription(group, storeArmed, updateArmed)}</small>
			{(storeArmed || setTarget || mutationOperation) && (
				<span className="pool-card-status-row">
					<span
						className={`pool-card-workflow ${
							storeArmed ? "record" : setTarget ? "set" : mutationOperation
						}`}
					>
						{storeArmed
							? "Record"
							: setTarget
								? "Set"
								: mutationOperation
									? canonicalPoolMutationOperation(mutationOperation)
									: null}
					</span>
				</span>
			)}
		</Button>
	);
}

function GroupShortcutList({
	visible,
	command,
	state,
	setInteraction,
	groupSelection,
	activateShortcut,
	poolPresentation,
	showId,
	viewOnly,
	mutationTarget,
	setTargetArmed,
}: {
	visible: readonly (ShortcutGroup | null)[];
	command: ReturnType<typeof useCommandLineSurface>;
	state: ReturnType<typeof useApp>["state"];
	setInteraction: ReturnType<typeof useSetInteraction>;
	groupSelection: ReturnType<typeof useGroupSelectionActions>;
	activateShortcut(group: ShortcutGroup | null, index: number): void;
	poolPresentation: PoolPresentationConfiguration;
	showId: string;
	viewOnly: boolean;
	mutationTarget: ReturnType<typeof poolMutationTarget>;
	setTargetArmed: boolean;
}) {
	return (
		<>
			{visible.map((group, index) => (
				<GroupShortcut
					key={group?.id ?? `empty-${index + 1}`}
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
					onClick={() => activateShortcut(group, index)}
					onDoubleClick={() => {
						if (group && !state.updateArmed) {
							const scope = setInteraction?.state?.scope;
							if (scope)
								void setInteraction.direct({
									type: "select_group_frozen",
									source: "touch",
									scope,
									group: { objectId: group.id, objectRevision: group.revision },
								});
							void groupSelection.selectFrozen(group);
						}
					}}
					onContextMenu={() => {
						if (!group || state.updateArmed) return;
						void (async () => {
							if (!setInteraction?.state) {
								await command.replace(`SET GROUP ${group.id}`, false);
								return;
							}
							if (setInteraction.state.phase === "idle") {
								if (!(await setInteraction.arm("context_menu"))) return;
							} else if (setInteraction.state.phase !== "set_armed") return;
							await setInteraction.chooseGroup(
								{ objectId: group.id, objectRevision: group.revision },
								"context_menu",
							);
						})();
					}}
					poolPresentation={poolPresentation}
					showId={showId}
					viewOnly={viewOnly}
				/>
			))}
		</>
	);
}

export function GroupStrip({
	active = true,
	viewOnly = false,
}: {
	active?: boolean;
	viewOnly?: boolean;
}) {
	const interactionActive = active && !viewOnly;
	useShowObjectView("group", active);
	const bootstrapReady = useBootstrapReady();
	const groupRecording = useGroupRecording();
	const commandLine = useCommandLineSurface({
		selection: true,
		enabled: interactionActive,
		observeCommand: true,
	});
	const storedGroups = usePortableGroups(active);
	const groupSelection = useGroupSelectionActions(interactionActive);
	const setInteraction = useSetInteraction();
	const { state, dispatch } = useApp();
	const { gridRef, slotCount } = useGroupShortcutCount(active);
	const poolPresentation = usePoolPresentationConfiguration();
	const showId = useActiveShowId() ?? "unresolved";
	const mutationTarget = poolMutationTarget(commandLine.text);
	const setTargetArmed =
		setInteraction?.state?.phase === "set_armed" ||
		/^SET$/iu.test(commandLine.text.trim());
	const [recordTarget, setRecordTarget] = useState<GroupRecordingTarget | null>(
		null,
	);
	const stored: readonly ShortcutGroup[] = fallbackGroups(
		bootstrapReady,
		storedGroups,
	);
	const visible = visibleGroups(stored, slotCount);
	const disarmRecord = () => {
		setRecordTarget(null);
		dispatch({ type: "SET_STORE_ARMED", value: false });
	};
	const recordGroup = async (
		target: GroupRecordingTarget,
		mode: RecordMode = "overwrite",
	) => {
		if (!groupRecording) return null;
		const outcome = await groupRecording.record({
			objectId: target.objectId,
			operation: mode,
			expectedObjectRevision: target.expectedObjectRevision,
		});
		if (outcome) await commandLine.reset();
		return outcome;
	};
	const selectGroup = (group: ShortcutGroup) => {
		if (setInteraction?.state?.phase === "set_armed") {
			void setInteraction.chooseGroup(
				{ objectId: group.id, objectRevision: group.revision },
				"touch",
			);
			return;
		}
		const scope = setInteraction?.state?.scope;
		if (scope)
			void setInteraction.direct({
				type: "select_group_live",
				source: "touch",
				scope,
				group: { objectId: group.id, objectRevision: group.revision },
			});
		const write = groupSelection.selectLive(group);
		if (!write) return;
		void write;
		void commandLine.replace(`GROUP ${group.id}`);
	};
	const activateShortcut = (group: ShortcutGroup | null, index: number) => {
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
			if (mutation.kind === "execute")
				void commandLine.execute(mutation.command);
			else void commandLine.replace(mutation.command, false);
			return;
		}
		if (group && !state.storeArmed) {
			selectGroup(group);
			return;
		}
		if (!state.storeArmed) return;
		if (group?.body.fixtures.length) {
			setRecordTarget(captureGroupRecordingTarget(group));
			return;
		}
		void recordGroup(
			group
				? captureGroupRecordingTarget(group)
				: emptyGroupRecordingTarget(id),
		);
		disarmRecord();
	};
	const recordExistingGroup = (mode: RecordMode) => {
		if (recordTarget) void recordGroup(recordTarget, mode);
		disarmRecord();
	};

	return (
		<section className="group-strip">
			<header>
				<b>Group shortcuts</b>
				<small>slots 1–{slotCount}</small>
			</header>
			<ButtonGrid
				ref={gridRef}
				square={false}
				className="card-pool group-shortcut-grid"
				style={{ "--group-shortcut-columns": slotCount } as React.CSSProperties}
			>
				<GroupShortcutList
					visible={visible}
					command={commandLine}
					state={state}
					setInteraction={setInteraction}
					groupSelection={groupSelection}
					activateShortcut={activateShortcut}
					poolPresentation={poolPresentation}
					showId={showId}
					viewOnly={viewOnly}
					mutationTarget={mutationTarget}
					setTargetArmed={setTargetArmed}
				/>
			</ButtonGrid>
			{recordTarget && (
				<RecordModeDialog
					target={recordTarget.label}
					onChoose={recordExistingGroup}
					onCancel={disarmRecord}
				/>
			)}
		</section>
	);
}

function visibleGroups(stored: readonly ShortcutGroup[], slotCount: number) {
	return Array.from(
		{ length: slotCount },
		(_, index) =>
			stored.find((group) => group.id === String(index + 1)) ?? null,
	);
}

function fallbackGroups(
	bootstrapReady: boolean,
	stored: readonly ShortcutGroup[],
) {
	if (bootstrapReady) return stored;
	return groups.map((group) => ({
		id: String(group.id),
		revision: 1,
		kind: "group" as const,
		updated_at: "",
		body: {
			name: group.name,
			fixtures: Array.from({ length: group.fixtures }, (_, index) =>
				String(index),
			),
		},
	}));
}
