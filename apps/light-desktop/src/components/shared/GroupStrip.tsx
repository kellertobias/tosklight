import { useLayoutEffect, useRef, useState } from "react";
import { groups } from "../../data/mockData";
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
import { usePortableGroups } from "../../features/showObjects/ShowObjectsState";
import { useShowObjectView } from "../../features/showObjects/ShowObjectsView";
import { useApp } from "../../state/AppContext";
import { Button } from "@tosklight/ui";
import { useCommandLineSurface } from "../control/commandLine/useCommandLineSurface";
import { requestUpdateTarget } from "../control/updateWorkflow";
import { ButtonGrid } from "@tosklight/ui/window-kit";
import { type RecordMode, RecordModeDialog } from "./RecordModeDialog";
import type { PoolPresentationConfiguration } from "../../api/types";
import {
	poolSurfaceKey,
	resolveConfiguredPoolPresentation,
	usePoolPresentationConfiguration,
} from "../../features/poolPresentation/poolPresentation";

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
	onClick,
	onDoubleClick,
	poolPresentation,
	showId,
}: {
	group: ShortcutGroup | null;
	index: number;
	selected: boolean;
	storeArmed: boolean;
	updateArmed: boolean;
	onClick: () => void;
	onDoubleClick: () => void;
	poolPresentation: PoolPresentationConfiguration;
	showId: string;
}) {
	const presentation = resolveConfiguredPoolPresentation(poolPresentation, {
		showId,
		surfaceKey: poolSurfaceKey(showId, "group"),
		objectType: "group",
		itemColorKey: group?.id,
		itemColor: group?.body.color,
		states: [
			...(selected ? (["selected"] as const) : []),
			...(group ? [] : (["empty"] as const)),
			...(storeArmed && !group ? (["record-target"] as const) : []),
			...(storeArmed && !group ? (["store-target"] as const) : []),
			...(updateArmed ? (["update-target"] as const) : []),
		],
	});
	return (
		<Button
			className={`group-card pool-cell ${presentation.className}`}
			style={presentation.style}
			aria-pressed={selected}
			onClick={onClick}
			onDoubleClick={onDoubleClick}
		>
			<span className="number">{index + 1}</span>
			<b>{group?.body.name ?? "Empty"}</b>
			<small>{shortcutDescription(group, storeArmed, updateArmed)}</small>
		</Button>
	);
}

export function GroupStrip({ active = true }: { active?: boolean }) {
	useShowObjectView("group", active);
	const bootstrapReady = useBootstrapReady();
	const groupRecording = useGroupRecording();
	const commandLine = useCommandLineSurface({
		selection: true,
		enabled: active,
		observeCommand: false,
	});
	const storedGroups = usePortableGroups(active);
	const groupSelection = useGroupSelectionActions(active);
	const { state, dispatch } = useApp();
	const { gridRef, slotCount } = useGroupShortcutCount(active);
	const poolPresentation = usePoolPresentationConfiguration();
	const showId = useActiveShowId() ?? "unresolved";
	const [recordTarget, setRecordTarget] = useState<GroupRecordingTarget | null>(
		null,
	);
	const stored: readonly ShortcutGroup[] = bootstrapReady
		? storedGroups
		: groups.map((group) => ({
				id: String(group.id),
				revision: 1,
				kind: "group",
				updated_at: "",
				body: {
					name: group.name,
					fixtures: Array.from({ length: group.fixtures }, (_, index) =>
						String(index),
					),
				},
			}));
	const visible = Array.from(
		{ length: slotCount },
		(_, index) =>
			stored.find((group) => group.id === String(index + 1)) ?? null,
	);
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
				{visible.map((group, index) => (
					<GroupShortcut
						key={group?.id ?? `empty-${index + 1}`}
						group={group}
						index={index}
						selected={commandLine.selectedGroupId === group?.id}
						storeArmed={state.storeArmed}
						updateArmed={state.updateArmed}
						onClick={() => activateShortcut(group, index)}
						onDoubleClick={() => {
							if (group && !state.updateArmed)
								void groupSelection.selectFrozen(group);
						}}
						poolPresentation={poolPresentation}
						showId={showId}
					/>
				))}
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
