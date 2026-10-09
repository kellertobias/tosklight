import {Button} from "@tosklight/ui";
import {useEffect,useState} from "react";
export function MvrInspectionProgress({operation,startedAt,file,onCancel}:{operation:"inspect"|"apply";startedAt:number;file:{name:string;size:number}|null;onCancel:()=>void}) {
    const [now,setNow]=useState(Date.now());
    useEffect(()=>{setNow(Date.now());const timer=globalThis.setInterval(()=>setNow(Date.now()),1000);return ()=>globalThis.clearInterval(timer);},[startedAt]);
    return <section role="status" aria-label="MVR operation progress">
        <strong>{operation==="inspect" ? "Inspecting MVR archive and fixture data" : "Applying MVR to the show"}</strong>
        <progress aria-label="MVR operation progress" style={{width:"100%"}}/>
        <p>{file ? `${file.name} · ${(file.size/1_000_000).toFixed(2)} MB · ` : ""}Elapsed {Math.max(0,Math.floor((now-startedAt)/1000))} s</p>
        <small>Remaining time cannot be estimated.</small>
        {operation==="inspect" ? <><p>The current show is unchanged during inspection.</p><Button onClick={onCancel}>Cancel inspection</Button></> : <p>Applying has started. Wait for completion before closing.</p>}
    </section>;
}
