import { useEffect, useState } from "react";
import { useServer } from "../../api/ServerContext";
import { useApp } from "../../state/AppContext";
import type { ScreenConfiguration } from "../../api/types";

const tauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> { const api = await import("@tauri-apps/api/core"); return api.invoke<T>(command, args); }

export function ScreensSetup() {
  const server = useServer(); const { state } = useApp();
  const [displays, setDisplays] = useState<Array<{ id: string; name: string }>>([]);
  useEffect(() => { if (tauri) void invoke<Array<{id:string;name:string}>>("list_console_displays").then(setDisplays); }, []);
  const update = (screen: ScreenConfiguration, changes: Partial<ScreenConfiguration>) => void server.saveScreen({ ...screen, ...changes });
  const create = () => { const id=crypto.randomUUID(); void server.saveScreen({ id, name:`Screen ${(server.screens?.screens.length ?? 0)+1}`, layout:{desks:state.desks,activeDeskId:state.activeDeskId}, show_dock:true,show_playbacks:true,playback_count:8,playback_rows:1,first_playback_slot:1,page_mode:"follow_main",show_page_controls:true,desired_open:false,display_id:null,bounds:null,fullscreen:false }); };
  const setOpen = async (screen:ScreenConfiguration, open:boolean) => { await server.saveScreen({...screen,desired_open:open}); if (tauri) await invoke(open?"open_console_screen":"close_console_screen",open?{screenId:screen.id,title:screen.name,displayId:screen.display_id,bounds:screen.bounds,fullscreen:screen.fullscreen}:{screenId:screen.id}); };
  if (!tauri) return <><h2>Screens</h2><p>Additional console screens are available in the ToskLight desktop app.</p></>;
  return <><h2>Screens</h2><p>Named operator surfaces restore on their assigned physical displays.</p><button onClick={create}>+ Add screen</button><div className="screens-setup-list">{(server.screens?.screens ?? []).map((screen)=><article key={screen.id}>
    <header><input value={screen.name} onChange={(e)=>update(screen,{name:e.target.value})}/><button onClick={()=>void setOpen(screen,!screen.desired_open)}>{screen.desired_open?"Close":"Open"}</button><button className="danger" onClick={()=>void (async()=>{await invoke("close_console_screen",{screenId:screen.id});await server.deleteScreen(screen.id);})()}>Delete</button></header>
    <div className="screen-settings-grid">
      <label><input type="checkbox" checked={screen.show_dock} onChange={(e)=>update(screen,{show_dock:e.target.checked})}/> Show dock</label>
      <label><input type="checkbox" checked={screen.show_playbacks} onChange={(e)=>update(screen,{show_playbacks:e.target.checked})}/> Show playbacks</label>
      <label>First page slot<input type="number" min="1" max={128-screen.playback_count} value={screen.first_playback_slot} onChange={(e)=>update(screen,{first_playback_slot:Number(e.target.value)})}/></label>
      <label>Playback count<input type="number" min="1" max={128-screen.first_playback_slot} value={screen.playback_count} onChange={(e)=>update(screen,{playback_count:Number(e.target.value)})}/></label>
      <label>Rows<input type="number" min="1" max={screen.playback_count} value={screen.playback_rows} onChange={(e)=>update(screen,{playback_rows:Number(e.target.value)})}/></label>
      <label>Page mode<select value={screen.page_mode} onChange={(e)=>update(screen,{page_mode:e.target.value as ScreenConfiguration["page_mode"]})}><option value="follow_main">Follow main</option><option value="independent">Independent</option></select></label>
      <label><input type="checkbox" checked={screen.show_page_controls} onChange={(e)=>update(screen,{show_page_controls:e.target.checked})}/> Show page controls</label>
      <label>Physical display<select value={screen.display_id ?? ""} onChange={(e)=>update(screen,{display_id:e.target.value||null})}><option value="">Choose when opened</option>{displays.map((display)=><option key={display.id} value={display.id}>{display.name}</option>)}</select></label>
      <label><input type="checkbox" checked={screen.fullscreen} onChange={(e)=>update(screen,{fullscreen:e.target.checked})}/> Fullscreen</label>
    </div>
  </article>)}</div></>;
}
