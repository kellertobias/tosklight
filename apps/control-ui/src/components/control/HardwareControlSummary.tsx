import { useRef, useState, type CSSProperties } from "react";
import { createPortal } from "react-dom";
import { useServer } from "../../api/ServerContext";
import { usePlaybackDeskView } from "../../features/playbackRuntime/PlaybackRuntimeView";
import { useApp } from "../../state/AppContext";
import { Button } from "../common";
import { ModalNumberInput } from "../input/ModalInputControls";
import { HighlightErrorAlert } from "./HighlightControls";
import { PlaybackPageMenu, PlaybackPageRenameDialog } from "./PlaybackPageDialogs";

export function HardwareControlSummary() {
  const server = useServer(); const { state, dispatch } = useApp(); const [pagesOpen, setPagesOpen] = useState(false); const [pageRenameOpen, setPageRenameOpen] = useState(false); const [timeInput, setTimeInput] = useState<"prog" | "cue" | null>(null); const [inputValue, setInputValue] = useState(""); const taps = useRef<Record<string, number[]>>({});
  const playbackDesk = usePlaybackDeskView();
  const bpms = server.configuration?.speed_groups_bpm ?? [120, 90, 60, 30, 15]; const prog = (server.configuration?.programmer_fade_millis ?? 3000) / 1000; const cue = (server.configuration?.sequence_master_fade_millis ?? 3000) / 1000; const page = playbackDesk?.active_page ?? server.playbacks?.active_page ?? state.playbackPage + 1;
  const openTime = (kind: "prog" | "cue", value: number) => { setTimeInput(kind); setInputValue(String(Number(value.toFixed(1)))); };
  const activePage = server.playbacks?.pages.find((item) => item.number === page) ?? null;
  const openPagesOrRename = () => { if (state.playbackSetArmed && activePage) { dispatch({ type: "SET_PLAYBACK_SET_ARMED", value: false }); setPageRenameOpen(true); } else setPagesOpen(true); };
  const submitTime = () => { const value = Math.max(0, Math.min(timeInput === "prog" ? 20 : 60, Number(inputValue))); if (Number.isFinite(value)) void server.setControlTiming(timeInput === "prog" ? { programmer_fade_millis: Math.round(value * 1000) } : { sequence_master_fade_millis: Math.round(value * 1000) }); setTimeInput(null); };
  const tap = (group: string, index: number) => { const now = performance.now(); const recent = [...(taps.current[group] ?? []), now].filter((time) => now - time < 3000).slice(-6); taps.current[group] = recent; if (recent.length < 2) return; const intervals = recent.slice(1).map((time, offset) => time - recent[offset]); const values = [...bpms] as [number,number,number,number,number]; values[index] = Math.round(60000 / (intervals.reduce((sum, value) => sum + value, 0) / intervals.length)); void server.setControlTiming({speed_groups_bpm: values}); };
  return <div className="hardware-control-summary"><div className="hardware-values"><Button onClick={()=>openTime("prog",prog)}><small>Prog Fade</small><b>{prog.toFixed(1)}s</b></Button><Button onClick={()=>openTime("cue",cue)}><small>Cue Fade</small><b>{cue.toFixed(1)}s</b></Button><Button aria-label={`Page ${page}`} onClick={openPagesOrRename}><small>Page</small><b>{page}</b></Button></div><div className="hardware-speed-groups">{(["A","B","C","D","E"] as const).map((group,index)=><Button style={{"--bpm":bpms[index]} as CSSProperties} key={group} onClick={()=>tap(group,index)}><b>{group}</b><span>{bpms[index]} BPM</span></Button>)}</div><HighlightErrorAlert message={server.highlightError} onDismiss={server.dismissHighlightError}/>{timeInput && createPortal(<div className="stacked-modal-layer" onPointerDown={(event)=>event.target===event.currentTarget&&setTimeInput(null)}><section className="nested-modal direct-value-modal" role="dialog" aria-modal="true"><Button className="modal-close" onClick={()=>setTimeInput(null)}>×</Button><h3>{timeInput==="prog"?"Prog. Fade":"Cue Fade"}</h3><strong>{inputValue||"0"}</strong><ModalNumberInput value={inputValue} onChange={setInputValue} onEnter={submitTime} onEscape={()=>setTimeInput(null)} replaceOnFirstInput/></section></div>,document.body)}<PlaybackPageMenu open={pagesOpen} onClose={()=>setPagesOpen(false)}/><PlaybackPageRenameDialog page={pageRenameOpen ? activePage : null} onClose={()=>setPageRenameOpen(false)}/></div>;
}
