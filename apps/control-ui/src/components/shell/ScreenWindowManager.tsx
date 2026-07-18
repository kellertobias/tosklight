import { useEffect, useRef } from "react";
import { useScreens } from "../../features/screens/ScreensContext";

export function ScreenWindowManager() {
  const server=useScreens();
  const screensRef=useRef(server.screens);
  const requestReconcile=useRef<()=>void>(()=>undefined);
  screensRef.current=server.screens;
  useEffect(()=>{if(!("__TAURI_INTERNALS__" in window))return;let cancelled=false;let running=false;let requested=false;
    const reconcile=async()=>{if(running){requested=true;return;}running=true;do{requested=false;const {invoke}=await import("@tauri-apps/api/core");const displays=await invoke<Array<{id:string}>>("list_console_displays");if(cancelled)break;const available=new Set(displays.map((display)=>display.id));for(const configured of screensRef.current?.screens??[]){if(cancelled)break;const screen=screensRef.current?.screens.find((item)=>item.id===configured.id);if(!screen)continue;if(screen.desired_open&&(!screen.display_id||available.has(screen.display_id)))await invoke("open_console_screen",{screenId:screen.id,title:screen.name,displayId:screen.display_id,bounds:screen.bounds,fullscreen:screen.fullscreen});else if(screen.desired_open)await invoke("hide_console_screen",{screenId:screen.id});else await invoke("close_console_screen",{screenId:screen.id});}}while(requested&&!cancelled);running=false;};
    requestReconcile.current=()=>void reconcile();requestReconcile.current();const timer=window.setInterval(requestReconcile.current,2000);return()=>{cancelled=true;requestReconcile.current=()=>undefined;window.clearInterval(timer);};
  },[]);
  useEffect(()=>{requestReconcile.current();},[server.screens]);
  return null;
}
