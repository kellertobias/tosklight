import { useLayoutEffect, useRef, useState } from "react";
import { groups } from "../../data/mockData";
import { useServer } from "../../api/ServerContext";
import { useApp } from "../../state/AppContext";
import { Button } from "../common";
import { ButtonGrid } from "../window-kit";
import { RecordModeDialog, type RecordMode } from "./RecordModeDialog";

const MIN_SHORTCUT_SIZE = 88;
const SHORTCUT_GAP = 2;

export function groupShortcutCount(width: number) {
  return Math.max(1, Math.floor((width + SHORTCUT_GAP) / (MIN_SHORTCUT_SIZE + SHORTCUT_GAP)));
}

export function GroupStrip() {
  const server = useServer();
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
  const stored = server.bootstrap ? server.groups : groups.map((group) => ({ id: String(group.id), body: { name: group.name, fixtures: Array.from({ length: group.fixtures }, (_, index) => String(index)) } }));
  const visible = Array.from({ length: slotCount }, (_, index) => stored.find((group) => group.id === String(index + 1)) ?? null);
  const recordTarget = stored.find((group) => group.id === recordGroup);
  const cancelRecording = () => { setRecordGroup(null); dispatch({ type: "SET_STORE_ARMED", value: false }); };
  const runCommand = (command: string, refresh = false) => { void server.executeCommandLine(command).then((ok) => { if (ok && refresh) void server.refresh(); }); };
  const recordGroupCommand = (id: string, mode: RecordMode = "overwrite") => runCommand(mode === "merge" ? `RECORD + GROUP ${id}` : `RECORD GROUP ${id}`, true);
  const recordExistingGroup = (mode: RecordMode) => { if (!recordTarget) return cancelRecording(); recordGroupCommand(recordTarget.id, mode); setRecordGroup(null); dispatch({ type: "SET_STORE_ARMED", value: false }); };
  return <section className="group-strip"><header><b>Group shortcuts</b><small>slots 1–{slotCount}</small></header><ButtonGrid ref={gridRef} className="card-pool group-shortcut-grid" style={{ "--group-shortcut-columns": slotCount } as React.CSSProperties}>{visible.map((group, index) => <Button onClick={() => { if (group && !state.storeArmed) return runCommand(`GROUP ${group.id}`); if (group) { if (!group.body.fixtures.length) { recordGroupCommand(group.id); dispatch({ type: "SET_STORE_ARMED", value: false }); } else { setRecordGroup(group.id); } return; } if (!state.storeArmed) return; recordGroupCommand(String(index + 1)); dispatch({ type: "SET_STORE_ARMED", value: false }); }} className={`group-card pool-cell ${server.selectedGroupId === group?.id ? "selected" : ""} ${group ? "" : "empty"} ${state.storeArmed && !group ? "store-target" : ""}`} key={group?.id ?? `empty-${index + 1}`}><span className="number">{index + 1}</span><b>{group?.body.name ?? "Empty"}</b><small>{group ? group.body.fixtures.length ? `${group.body.fixtures.length} fixtures` : "Group is empty" : state.storeArmed ? "Tap to record" : "Press Rec first"}</small></Button>)}</ButtonGrid>{recordTarget && <RecordModeDialog target={recordTarget.body.name ?? `Group ${recordTarget.id}`} onChoose={recordExistingGroup} onCancel={cancelRecording}/>}</section>;
}
