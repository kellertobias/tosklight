import { useEffect, useRef, useState } from "react";
import { useServer } from "../../api/ServerContext";
import { useApp } from "../../state/AppContext";
import type { ScreenConfiguration } from "../../api/types";
import { Button, FormLayout, NumberField, SelectField, SwitchField, TextField } from "../common";
import { createScreenConfiguration } from "./screenConfiguration";

const tauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> { const api = await import("@tauri-apps/api/core"); return api.invoke<T>(command, args); }

export function ScreenSettingsCard({ screen, displays, save, remove }: { screen: ScreenConfiguration; displays: Array<{ id: string; name: string }>; save: (screen: ScreenConfiguration) => Promise<void>; remove: (screen: ScreenConfiguration) => Promise<void> }) {
  const [draft, setDraft] = useState(screen);
  const draftRef = useRef(screen);
  const saveQueue = useRef(Promise.resolve());
  const pending = useRef(0);
  useEffect(() => { if (pending.current === 0) { draftRef.current = screen; setDraft(screen); } }, [screen]);
  const update = (changes: Partial<ScreenConfiguration>) => {
    const next = { ...draftRef.current, ...changes };
    draftRef.current = next;
    setDraft(next);
    pending.current += 1;
    saveQueue.current = saveQueue.current.then(() => save(next)).finally(() => { pending.current -= 1; });
  };
  return <article className="screen-settings-card">
    <header className="screen-settings-header"><TextField aria-label="Screen name" value={draft.name} onChange={(event)=>update({name:event.target.value})}/><div className="screen-settings-actions"><Button variant={draft.desired_open?"warning":"success"} onClick={()=>update({desired_open:!draft.desired_open})}>{draft.desired_open?"Close Screen":"Open Screen"}</Button><Button variant="danger" onClick={()=>void remove(draft)}>Remove Screen</Button></div></header>
    <div className="screen-settings-columns">
      <section><h3>Layout</h3><div className="screen-settings-fields"><SwitchField label="Show Dock" checked={draft.show_dock} onChange={(event)=>update({show_dock:event.target.checked})}/><SwitchField label="Show Playbacks" checked={draft.show_playbacks} onChange={(event)=>update({show_playbacks:event.target.checked})}/><SwitchField label="Show Page Controls" checked={draft.show_page_controls} onChange={(event)=>update({show_page_controls:event.target.checked})}/></div></section>
      <section><h3>Placement</h3><div className="screen-settings-fields"><SelectField label="Physical Display" value={draft.display_id ?? ""} onChange={(value)=>update({display_id:value||null})} options={[{value:"",label:"Choose when opened"},...displays.map((display)=>({value:display.id,label:display.name}))]}/><SwitchField label="Fullscreen" checked={draft.fullscreen} onChange={(event)=>update({fullscreen:event.target.checked})}/></div></section>
      <section><h3>Playbacks</h3><div className="screen-settings-fields"><NumberField label="First Playback Number" min="1" max={128-draft.playback_count} value={draft.first_playback_slot} onChange={(event)=>update({first_playback_slot:Number(event.target.value)})}/><NumberField label="Playback Count" min="1" max={128-draft.first_playback_slot} value={draft.playback_count} onChange={(event)=>update({playback_count:Number(event.target.value)})}/><NumberField label="Rows" min="1" max={draft.playback_count} value={draft.playback_rows} onChange={(event)=>update({playback_rows:Number(event.target.value)})}/><SelectField label="Page Mode" value={draft.page_mode} onChange={(value)=>update({page_mode:value})} options={[{value:"follow_main",label:"Follow Main"},{value:"independent",label:"Dedicated Page"}]}/></div></section>
    </div>
  </article>;
}

export function ScreensSetup() {
  const server = useServer(); const { state, dispatch } = useApp();
  const [displays, setDisplays] = useState<Array<{ id: string; name: string }>>([]);
  const [deskButtons, setDeskButtons] = useState(3);
  const [deskName, setDeskName] = useState("");
  const [deskAlias, setDeskAlias] = useState("");
  useEffect(() => { if (tauri) void invoke<Array<{id:string;name:string}>>("list_console_displays").then(setDisplays); }, []);
  useEffect(() => { const desk=server.session?.desk;if(!desk)return;setDeskButtons(desk.buttons);setDeskName(desk.name);setDeskAlias(desk.osc_alias);dispatch({type:"SET_PLAYBACK_LAYOUT",columns:desk.columns,rows:desk.rows}); }, [server.session?.desk, dispatch]);
  const create = () => void server.saveScreen(createScreenConfiguration(server.screens?.screens ?? [], { desks: state.desks, activeDeskId: state.activeDeskId }));
  const remove = async (screen: ScreenConfiguration) => { await invoke("close_console_screen", { screenId: screen.id }); await server.deleteScreen(screen.id); };
  return <div className="screens-playback-setup"><header><div><h2>Screens & playback</h2><p>Configure the default desk surface, then add optional operator screens.</p></div>{tauri && <Button variant="primary" onClick={create}>+ Add screen</Button>}</header><div className="screens-setup-list"><article className="default-screen-settings"><header><div><b>Default screen</b><small>Primary desk window</small></div></header><FormLayout className="screen-settings-grid" columns={3} minColumnWidth={180}><TextField label="Name" value={deskName} onChange={(event)=>setDeskName(event.target.value)}/><TextField label="OSC alias" value={deskAlias} onChange={(event)=>setDeskAlias(event.target.value)}/><NumberField label="Playbacks per row" min="1" max="32" value={state.playbackColumns} onChange={(event)=>dispatch({type:"SET_PLAYBACK_LAYOUT",columns:Number(event.target.value),rows:state.playbackRows})}/><NumberField label="Rows" min="1" max="3" value={state.playbackRows} onChange={(event)=>dispatch({type:"SET_PLAYBACK_LAYOUT",columns:state.playbackColumns,rows:Number(event.target.value)})}/><NumberField label="Visible buttons" min="0" max="3" value={deskButtons} onChange={(event)=>setDeskButtons(Number(event.target.value))}/></FormLayout><footer><small>{state.playbackColumns*state.playbackRows} playback slots · OSC /light/{deskAlias||"desk"}/</small><Button variant="primary" onClick={()=>server.session?.desk&&void server.updateControlDesk({...server.session.desk,name:deskName,osc_alias:deskAlias,columns:state.playbackColumns,rows:state.playbackRows,buttons:deskButtons})}>Save default screen</Button></footer></article>{server.bootstrap?.desks.filter((desk)=>desk.id!==server.session?.desk.id).map((desk)=><article className="available-desk" key={desk.id}><b>{desk.name}</b><span>/{desk.osc_alias}/ · {desk.columns}×{desk.rows} · {desk.buttons} buttons</span><Button onClick={()=>server.selectControlDesk(desk.id)}>Use as default screen</Button></article>)}{!tauri&&<p>Additional console screens are available in the ToskLight desktop app.</p>}{tauri&&(server.screens?.screens ?? []).map((screen)=><ScreenSettingsCard key={screen.id} screen={screen} displays={displays} save={server.saveScreen} remove={remove}/>)}</div></div>;
}
