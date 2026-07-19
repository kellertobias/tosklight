import { useLayoutEffect, useRef, useState } from "react";
import { useServer } from "../../api/ServerContext";
import { useCommandLineSurface } from "../control/commandLine/useCommandLineSurface";
import { requestUpdateTarget } from "../control/updateWorkflow";
import { groups } from "../../data/mockData";
import { useShowObjectView } from "../../features/showObjects/ShowObjectsView";
import { useGroups } from "../../features/server/useShowObjectsState";
import { useApp } from "../../state/AppContext";
import { Button } from "../common";
import { ButtonGrid } from "../window-kit";
import { type RecordMode, RecordModeDialog } from "./RecordModeDialog";

const MIN_SHORTCUT_SIZE = 88;
const SHORTCUT_GAP = 2;

type ShortcutGroup = ReturnType<typeof useGroups>[number];

export function groupShortcutCount(width: number) {
	return Math.max(
		1,
		Math.floor(
			(width + SHORTCUT_GAP) / (MIN_SHORTCUT_SIZE + SHORTCUT_GAP),
		),
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
}: {
	group: ShortcutGroup | null;
	index: number;
	selected: boolean;
	storeArmed: boolean;
	updateArmed: boolean;
	onClick: () => void;
	onDoubleClick: () => void;
}) {
	return (
		<Button
			className={`group-card pool-cell ${selected ? "selected" : ""} ${group ? "" : "empty"} ${storeArmed && !group ? "store-target" : ""} ${updateArmed ? "update-target" : ""}`}
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
	const server = useServer();
	const commandLine = useCommandLineSurface({
		selection: true,
		enabled: active,
		observeCommand: false,
	});
	const storedGroups = useGroups(server.playbacks);
	const { state, dispatch } = useApp();
	const { gridRef, slotCount } = useGroupShortcutCount(active);
	const [recordGroupId, setRecordGroupId] = useState<string | null>(null);
	const stored: readonly ShortcutGroup[] = server.bootstrap
		? storedGroups
		: groups.map((group) => ({
				id: String(group.id),
				revision: 1,
				kind: "group",
				updated_at: "",
				body: {
					name: group.name,
					fixtures: Array.from(
						{ length: group.fixtures },
						(_, index) => String(index),
					),
				},
			}));
	const visible = Array.from(
		{ length: slotCount },
		(_, index) =>
			stored.find((group) => group.id === String(index + 1)) ?? null,
	);
	const recordTarget = stored.find((group) => group.id === recordGroupId);
	const disarmRecord = () => {
		setRecordGroupId(null);
		dispatch({ type: "SET_STORE_ARMED", value: false });
	};
	const recordGroup = async (
		id: string,
		mode: RecordMode = "overwrite",
	) => {
		const command =
			mode === "merge" ? `RECORD + GROUP ${id}` : `RECORD GROUP ${id}`;
		if (await commandLine.execute(command)) await server.refreshGroup(id);
	};
	const selectGroup = (id: string) => {
		void server.selectionGesture({ type: "live_group", group_id: id });
		void commandLine.replace(`GROUP ${id}`);
	};
	const activateShortcut = (group: ShortcutGroup | null, index: number) => {
		const id = group?.id ?? String(index + 1);
		if (state.updateArmed) {
			requestUpdateTarget({ family: { type: "group" }, object_id: id });
			return;
		}
		if (group && !state.storeArmed) {
			selectGroup(group.id);
			return;
		}
		if (!state.storeArmed) return;
		if (group?.body.fixtures.length) {
			setRecordGroupId(group.id);
			return;
		}
		void recordGroup(id);
		disarmRecord();
	};
	const recordExistingGroup = (mode: RecordMode) => {
		if (recordTarget) void recordGroup(recordTarget.id, mode);
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
								void server.selectGroup(group.id, true);
						}}
					/>
				))}
			</ButtonGrid>
			{recordTarget && (
				<RecordModeDialog
					target={recordTarget.body.name ?? `Group ${recordTarget.id}`}
					onChoose={recordExistingGroup}
					onCancel={disarmRecord}
				/>
			)}
		</section>
	);
}
