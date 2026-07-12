import { useEffect } from "react";
import { useServer } from "../../api/ServerContext";

export function ScreenWindowManager() {
  const server=useServer();
  useEffect(()=>{ if (!("__TAURI_INTERNALS__" in window) || !server.screens) return; let cancelled=false;
    const reconcile=async()=>{const {invoke}=await import("@tauri-apps/api/core");const displays=await invoke<Array<{id:string}>>("list_console_displays");if(cancelled)return;const available=new Set(displays.map((d)=>d.id));for(const screen of server.screens!.screens){if(screen.desired_open&&(!screen.display_id||available.has(screen.display_id)))await invoke("open_console_screen",{screenId:screen.id,title:screen.name,displayId:screen.display_id,bounds:screen.bounds,fullscreen:screen.fullscreen});else if(screen.desired_open)await invoke("hide_console_screen",{screenId:screen.id});else await invoke("close_console_screen",{screenId:screen.id});}};
    void reconcile();const timer=window.setInterval(()=>void reconcile(),2000);return()=>{cancelled=true;window.clearInterval(timer);};
  },[server.screens]);
  return null;
}
