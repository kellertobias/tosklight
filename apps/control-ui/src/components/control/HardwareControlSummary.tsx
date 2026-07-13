import { useState, type CSSProperties } from "react";
import { createPortal } from "react-dom";
import { useServer } from "../../api/ServerContext";
import { useApp } from "../../state/AppContext";
import { Button } from "../common";

export function HardwareControlSummary() {
  const server = useServer(); const { state, dispatch } = useApp(); const [pagesOpen, setPagesOpen] = useState(false);
  const bpms = server.configuration?.speed_groups_bpm ?? [120, 90, 60, 30, 15]; const prog = (server.configuration?.programmer_fade_millis ?? 3000) / 1000; const cue = (server.configuration?.sequence_master_fade_millis ?? 3000) / 1000; const page = server.playbacks?.active_page ?? state.playbackPage + 1;
  return <div className="hardware-control-summary"><div className="hardware-values"><span><small>Prog Fade</small><b>{prog.toFixed(1)}s</b></span><span><small>Cue Fade</small><b>{cue.toFixed(1)}s</b></span><Button onClick={() => setPagesOpen(true)}><small>Page</small><b>{page}</b></Button></div><div className="hardware-speed-groups">{(["A","B","C","D","E"] as const).map((group,index)=><Button style={{"--bpm":bpms[index]} as CSSProperties} className={state.speedGroup===group?"selected":""} key={group} onClick={()=>dispatch({type:"SET_SPEED_GROUP",value:group})}><b>{group}</b><span>{bpms[index]} BPM</span></Button>)}</div>{pagesOpen && createPortal(<div className="stacked-modal-layer" onPointerDown={(event)=>event.target===event.currentTarget&&setPagesOpen(false)}><section className="nested-modal playback-page-modal"><Button className="modal-close" onClick={()=>setPagesOpen(false)}>×</Button><h3>Playback pages</h3><div>{(server.playbacks?.pages ?? []).map((item)=><Button className={item.number===page?"active":""} key={item.number} onClick={()=>{dispatch({type:"SET_PLAYBACK_PAGE",page:item.number-1});void server.setPlaybackPage(item.number);setPagesOpen(false);}}><strong>{item.number}</strong><span>{item.name}</span></Button>)}</div></section></div>,document.body)}</div>;
}
