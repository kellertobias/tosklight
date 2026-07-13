import { useEffect, useState } from "react";
import { useServer } from "../../api/ServerContext";
import { useApp } from "../../state/AppContext";
import type { ScreenConfiguration } from "../../api/types";
import { Button, NumberField, SelectField, SwitchField, TextField } from "../common";

const tauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> { const api = await import("@tauri-apps/api/core"); return api.invoke<T>(command, args); }

export function ScreensSetup() {
  const server = useServer(); const { state, dispatch } = useApp();
  const [displays, setDisplays] = useState<Array<{ id: string; name: string }>>([]);
  const [deskButtons, setDeskButtons] = useState(3);
  const [deskName, setDeskName] = useState("");
  const [deskAlias, setDeskAlias] = useState("");
  useEffect(() => { if (tauri) void invoke<Array<{id:string;name:string}>>("list_console_displays").then(setDisplays); }, []);
  useEffect(() => { const desk=server.session?.desk;if(!desk)return;setDeskButtons(desk.buttons);setDeskName(desk.name);setDeskAlias(desk.osc_alias);dispatch({type:"SET_PLAYBACK_LAYOUT",columns:desk.columns,rows:desk.rows}); }, [server.session?.desk, dispatch]);
  const update = (screen: ScreenConfiguration, changes: Partial<ScreenConfiguration>) => void server.saveScreen({ ...screen, ...changes });
  const create = () => { const id=crypto.randomUUID(); void server.saveScreen({ id, name:`Screen ${(server.screens?.screens.length ?? 0)+1}`, layout:{desks:state.desks,activeDeskId:state.activeDeskId}, show_dock:true,show_playbacks:true,playback_count:8,playback_rows:1,first_playback_slot:1,page_mode:"follow_main",show_page_controls:true,desired_open:false,display_id:null,bounds:null,fullscreen:false }); };
  const setOpen = async (screen:ScreenConfiguration, open:boolean) => { await server.saveScreen({...screen,desired_open:open}); if (tauri) await invoke(open?"open_console_screen":"close_console_screen",open?{screenId:screen.id,title:screen.name,displayId:screen.display_id,bounds:screen.bounds,fullscreen:screen.fullscreen}:{screenId:screen.id}); };
  return <div className="screens-playback-setup"><header><div><h2>Screens & playback</h2><p>Configure the default desk surface, then add optional operator screens.</p></div>{tauri && <Button variant="primary" onClick={create}>+ Add screen</Button>}</header><div className="screens-setup-list"><article className="default-screen-settings"><header><div><b>Default screen</b><small>Primary desk window</small></div></header><div className="screen-settings-grid"><TextField label="Name" value={deskName} onChange={(event)=>setDeskName(event.target.value)}/><TextField label="OSC alias" value={deskAlias} onChange={(event)=>setDeskAlias(event.target.value)}/><NumberField label="Playbacks per row" min="1" max="32" value={state.playbackColumns} onChange={(event)=>dispatch({type:"SET_PLAYBACK_LAYOUT",columns:Number(event.target.value),rows:state.playbackRows})}/><NumberField label="Rows" min="1" max="3" value={state.playbackRows} onChange={(event)=>dispatch({type:"SET_PLAYBACK_LAYOUT",columns:state.playbackColumns,rows:Number(event.target.value)})}/><NumberField label="Visible buttons" min="0" max="3" value={deskButtons} onChange={(event)=>setDeskButtons(Number(event.target.value))}/></div><footer><small>{state.playbackColumns*state.playbackRows} playback slots · OSC /light/{deskAlias||"desk"}/</small><Button variant="primary" onClick={()=>server.session?.desk&&void server.updateControlDesk({...server.session.desk,name:deskName,osc_alias:deskAlias,columns:state.playbackColumns,rows:state.playbackRows,buttons:deskButtons})}>Save default screen</Button></footer></article>{server.bootstrap?.desks.filter((desk)=>desk.id!==server.session?.desk.id).map((desk)=><article className="available-desk" key={desk.id}><b>{desk.name}</b><span>/{desk.osc_alias}/ · {desk.columns}×{desk.rows} · {desk.buttons} buttons</span><Button onClick={()=>server.selectControlDesk(desk.id)}>Use as default screen</Button></article>)}{!tauri&&<p>Additional console screens are available in the ToskLight desktop app.</p>}{tauri&&(server.screens?.screens ?? []).map((screen)=><article key={screen.id}>
    <header><TextField aria-label="Screen name" value={screen.name} onChange={(e)=>update(screen,{name:e.target.value})}/><Button variant={screen.desired_open?"warning":"success"} onClick={()=>void setOpen(screen,!screen.desired_open)}>{screen.desired_open?"Close":"Open"}</Button><Button variant="danger" onClick={()=>void (async()=>{await invoke("close_console_screen",{screenId:screen.id});await server.deleteScreen(screen.id);})()}>Delete</Button></header>
    <div className="screen-settings-grid">
      <SwitchField label="Show dock" checked={screen.show_dock} onChange={(e)=>update(screen,{show_dock:e.target.checked})}/>
      <SwitchField label="Show playbacks" checked={screen.show_playbacks} onChange={(e)=>update(screen,{show_playbacks:e.target.checked})}/>
      <NumberField label="First page slot" min="1" max={128-screen.playback_count} value={screen.first_playback_slot} onChange={(e)=>update(screen,{first_playback_slot:Number(e.target.value)})}/>
      <NumberField label="Playback count" min="1" max={128-screen.first_playback_slot} value={screen.playback_count} onChange={(e)=>update(screen,{playback_count:Number(e.target.value)})}/>
      <NumberField label="Rows" min="1" max={screen.playback_count} value={screen.playback_rows} onChange={(e)=>update(screen,{playback_rows:Number(e.target.value)})}/>
      <SelectField label="Page mode" value={screen.page_mode} onChange={(value)=>update(screen,{page_mode:value})} options={[{value:"follow_main",label:"Follow main"},{value:"independent",label:"Independent"}]}/>
      <SwitchField label="Show page controls" checked={screen.show_page_controls} onChange={(e)=>update(screen,{show_page_controls:e.target.checked})}/>
      <SelectField label="Physical display" value={screen.display_id ?? ""} onChange={(value)=>update(screen,{display_id:value||null})} options={[{value:"",label:"Choose when opened"},...displays.map((display)=>({value:display.id,label:display.name}))]}/>
      <SwitchField label="Fullscreen" checked={screen.fullscreen} onChange={(e)=>update(screen,{fullscreen:e.target.checked})}/>
    </div>
  </article>)}</div></div>;
}
