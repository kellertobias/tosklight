import { useLayoutEffect, useRef, useState } from "react";
import { groups } from "../../data/mockData";
import { useServer } from "../../api/ServerContext";
import { useApp } from "../../state/AppContext";
import { Button } from "../common";
import { ButtonGrid } from "../window-kit";
import { RecordModeDialog, type RecordMode } from "./RecordModeDialog";
import { requestUpdateTarget } from "../control/updateWorkflow";
import { useShowObjectView } from "../../features/showObjects/ShowObjectsView";
import { useGroups } from "../../features/server/useShowObjectsState";

const MIN_SHORTCUT_SIZE = 88;
const SHORTCUT_GAP = 2;

export function groupShortcutCount(width: number) {
  return Math.max(1, Math.floor((width + SHORTCUT_GAP) / (MIN_SHORTCUT_SIZE + SHORTCUT_GAP)));
}

export function GroupStrip({ active = true }: { active?: boolean }) {
	useShowObjectView("group", active);
  const server = useServer();
  const storedGroups = useGroups(server.playbacks);
  const { state, dispatch } = useApp();
  const gridRef = useRef<HTMLDivElement>(null);
  const [slotCount, setSlotCount] = useState(10);
  const [recordGroup, setRecordGroup] = useState<string | null>(null);
  useLayoutEffect(() => {
    const grid = gridRef.current;
    if (!grid) return;
    const measure = () => setSlotCount(groupShortcutCount(grid.clientWidth));
    measure();
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(measure);
    observer.observe(grid);
    return () => observer.disconnect();
  }, []);
  const stored = server.bootstrap ? storedGroups : groups.map((group) => ({ id: String(group.id), body: { name: group.name, fixtures: Array.from({ length: group.fixtures }, (_, index) => String(index)) } }));
  const visible = Array.from({ length: slotCount }, (_, index) => stored.find((group) => group.id === String(index + 1)) ?? null);
  const recordTarget = stored.find((group) => group.id === recordGroup);
  const cancelRecording = () => { setRecordGroup(null); dispatch({ type: "SET_STORE_ARMED", value: false }); };
  const selectShortcutGroup = (id: string) => { void server.selectionGesture({ type: "live_group", group_id: id }); server.setCommandLine(`GROUP ${id}`); };
  const recordGroupCommand = (id: string, mode: RecordMode = "overwrite") => {
    const command = mode === "merge" ? `RECORD + GROUP ${id}` : `RECORD GROUP ${id}`;
    void server.executeCommandLine(command).then((ok) => {
      if (ok) void server.refreshGroup(id);
    });
  };
  const recordExistingGroup = (mode: RecordMode) => { if (!recordTarget) return cancelRecording(); recordGroupCommand(recordTarget.id, mode); setRecordGroup(null); dispatch({ type: "SET_STORE_ARMED", value: false }); };
  return <section className="group-strip"><header><b>Group shortcuts</b><small>slots 1–{slotCount}</small></header><ButtonGrid ref={gridRef} className="card-pool group-shortcut-grid" style={{ "--group-shortcut-columns": slotCount } as React.CSSProperties}>{visible.map((group, index) => <Button onDoubleClick={() => group && !state.updateArmed && void server.selectGroup(group.id, true)} onClick={() => { if (state.updateArmed) { requestUpdateTarget({ family: { type: "group" }, object_id: group?.id ?? String(index + 1) }); return; } if (group && !state.storeArmed) return selectShortcutGroup(group.id); if (group) { if (!group.body.fixtures.length) { recordGroupCommand(group.id); dispatch({ type: "SET_STORE_ARMED", value: false }); } else { setRecordGroup(group.id); } return; } if (!state.storeArmed) return; recordGroupCommand(String(index + 1)); dispatch({ type: "SET_STORE_ARMED", value: false }); }} className={`group-card pool-cell ${server.selectedGroupId === group?.id ? "selected" : ""} ${group ? "" : "empty"} ${state.storeArmed && !group ? "store-target" : ""} ${state.updateArmed ? "update-target" : ""}`} key={group?.id ?? `empty-${index + 1}`}><span className="number">{index + 1}</span><b>{group?.body.name ?? "Empty"}</b><small>{state.updateArmed ? "Touch to check Update eligibility" : group ? group.body.fixtures.length ? `${group.body.fixtures.length} fixtures` : "Group is empty" : state.storeArmed ? "Tap to record" : "Press Rec first"}</small></Button>)}</ButtonGrid>{recordTarget && <RecordModeDialog target={recordTarget.body.name ?? `Group ${recordTarget.id}`} onChoose={recordExistingGroup} onCancel={cancelRecording}/>}</section>;
}
