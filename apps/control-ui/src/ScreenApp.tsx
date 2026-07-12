import { useEffect, useRef, useState } from "react";
import { ServerProvider, useServer } from "./api/ServerContext";
import { AppProvider, useApp } from "./state/AppContext";
import { LeftDock } from "./components/shell/LeftDock";
import { WorkspaceView } from "./components/shell/WorkspaceView";
import { PlaybackFaderBank } from "./components/control/PlaybackFaderBank";
import type { ScreenConfiguration } from "./api/types";

function ScreenPageControls({ screen, page }: { screen: ScreenConfiguration; page: number }) {
  const server = useServer();
  const [picker, setPicker] = useState(false);
  const setPage = (next: number) => screen.page_mode === "independent" && void server.setScreenPage(screen.id, next);
  return <div className="screen-page-controls">
    <button disabled={screen.page_mode !== "independent" || page <= 1} onClick={() => setPage(page - 1)}>▲ PAGE UP</button>
    <button onClick={() => screen.page_mode === "independent" && setPicker(true)}><strong>{page}</strong><span>{server.playbacks?.pages.find((item) => item.number === page)?.name ?? `Page ${page}`}</span></button>
    <button disabled={screen.page_mode !== "independent" || page >= 127} onClick={() => setPage(page + 1)}>PAGE DOWN ▼</button>
    {picker && <div className="screen-page-picker"><button onClick={() => setPicker(false)}>×</button>{(server.playbacks?.pages ?? []).map((item) => <button className={item.number === page ? "active" : ""} key={item.number} onClick={() => { setPage(item.number); setPicker(false); }}>{item.number} · {item.name}</button>)}</div>}
  </div>;
}

function ScreenSurface({ id }: { id: string }) {
  const server = useServer();
  const { state, dispatch } = useApp();
  const screen = server.screens?.screens.find((item) => item.id === id);
  const hydrated = useRef(false);
  useEffect(() => { if (!screen || hydrated.current) return; dispatch({ type: "HYDRATE_LAYOUT", desks: screen.layout.desks, activeDeskId: screen.layout.activeDeskId }); hydrated.current = true; }, [screen, dispatch]);
  useEffect(() => { if (!screen || !hydrated.current) return; const timer = window.setTimeout(() => void server.saveScreen({ ...screen, layout: { desks: state.desks, activeDeskId: state.activeDeskId } }), 600); return () => window.clearTimeout(timer); }, [state.desks, state.activeDeskId]);
  useEffect(()=>{if(!screen||!("__TAURI_INTERNALS__" in window))return;let cleanups:Array<()=>void>=[];let timer:number|undefined;let shuttingDown=false;void Promise.all([import("@tauri-apps/api/window"),import("@tauri-apps/api/event")]).then(async([{getCurrentWindow,currentMonitor},{listen}])=>{const current=getCurrentWindow();cleanups.push(await listen("app-shutting-down",()=>{shuttingDown=true;}));const persist=()=>{window.clearTimeout(timer);timer=window.setTimeout(async()=>{const position=await current.outerPosition();const size=await current.outerSize();const scale=await current.scaleFactor();const fullscreen=await current.isFullscreen();const monitor=await currentMonitor();const display_id=monitor?`${monitor.name??"Display"}|${monitor.position.x},${monitor.position.y}|${monitor.size.width}x${monitor.size.height}`:screen.display_id;void server.saveScreen({...screen,display_id,bounds:{x:position.x/scale,y:position.y/scale,width:size.width/scale,height:size.height/scale},fullscreen});},300);};cleanups.push(await current.onMoved(persist),await current.onResized(persist),await current.onCloseRequested(async(event)=>{event.preventDefault();if(!shuttingDown)await server.saveScreen({...screen,desired_open:false});await current.destroy();}));persist();});return()=>{window.clearTimeout(timer);cleanups.forEach((cleanup)=>cleanup());};},[screen?.id]);
  if (!screen) return <main className="screen-loading">Loading screen…</main>;
  const page = server.screens?.active_pages[id] ?? server.playbacks?.active_page ?? 1;
  return <div className={`screen-shell ${screen.show_dock ? "with-dock" : ""} ${screen.show_playbacks ? "with-playbacks" : ""}`}>
    {screen.show_dock && <LeftDock />}
    <WorkspaceView />
    {screen.show_playbacks && <section className="screen-playbacks">
      <PlaybackFaderBank pageNumber={page} firstSlot={screen.first_playback_slot} count={screen.playback_count} rows={screen.playback_rows}/>
      {screen.show_page_controls && <ScreenPageControls screen={screen} page={page}/>} 
    </section>}
  </div>;
}

export function ScreenApp({ id }: { id: string }) { return <ServerProvider><AppProvider><ScreenSurface id={id}/></AppProvider></ServerProvider>; }
