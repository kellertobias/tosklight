import test from "node:test";
import assert from "node:assert/strict";
import {mkdtemp,writeFile,readFile,stat,readdir,rm,open} from "node:fs/promises";
import path from "node:path";
import {artifactPaths} from "./artifact-paths.mjs";
import {PHASE_FIELDS} from "./desktop-performance-576-phases.mjs";
import {APPLICATION_FIELDS} from "./desktop-performance-576-tick-trace.mjs";
import {CPU_LIMITS,CPU_STAGE_IDS,CPU_SUBSTAGE_IDS,CPU_WORKER_IDS,CPU_ROW_METRICS,CPU_WORKER_METRICS,CPU_CLEANUP_METRICS,parseCpuDiagnostics,validateCpuDiagnostics,readCpuDiagnostics,writeCpuDiagnostics} from "./desktop-performance-576-cpu.mjs";

const epoch=Date.parse("2026-10-17T10:00:00Z"),window={startedAt:epoch,endedAt:epoch+60_000};
function line(seq,message,fields,at=100){return new Date(epoch+at).toISOString()+" INFO "+(seq===null?"":"output_tick{tick_seq="+seq+"}: ")+"light_headless_runtime: "+message+" "+fields+"\n";}
function source(){
 const a=n=>Array(n).fill(1),ns=n=>Array.from({length:n},(_,i)=>1000*(i+1));
 return {
  stage_calls:a(14),stage_cpu_valid_calls:a(14),stage_cpu_unavailable_calls:Array(14).fill(0),stage_cpu_invalid_calls:Array(14).fill(0),stage_cpu_nanos:ns(14),stage_counter_read_nanos:Array(14).fill(20),
  substage_calls:a(7),substage_wall_nanos:ns(7).map(n=>n*10),substage_cpu_valid_calls:a(7),substage_cpu_unavailable_calls:Array(7).fill(0),substage_cpu_invalid_calls:Array(7).fill(0),substage_cpu_nanos:ns(7),substage_counter_read_nanos:Array(7).fill(20),
  worker_calls:a(3),worker_jobs:Array(3).fill(2),worker_cpu_valid_jobs:Array(3).fill(2),worker_cpu_unavailable_jobs:Array(3).fill(0),worker_cpu_invalid_jobs:Array(3).fill(0),worker_cpu_nanos:ns(3),worker_counter_read_nanos:Array(3).fill(40),worker_job_wall_nanos:Array(3).fill(20_000),worker_job_wall_max_nanos:Array(3).fill(10_000),worker_section_to_job_max_nanos:Array(3).fill(2_000),worker_uncovered_sort_calls:a(3),
 };
}
function cpu(seq,data=source(),at=98){return line(seq,"output preparation cpu summary","schema_version=1 "+Object.entries(data).map(([key,value])=>key+"="+JSON.stringify(value)).join(" "),at);}
function tick(seq=1,{at=100,miss=false,pathCode=0,data=source(),summary=true,nested=pathCode!==1}={}){
 const phase=PHASE_FIELDS.map((key,i)=>key+"="+(10+i)).join(" "),app=APPLICATION_FIELDS.map((key,i)=>key+"="+(1000+i)).join(" ");
 return [
  ...(nested?[line(seq,"bounded live output nested phase sample","generation=Some(123) show_revision=Some(99) sampled_at_millis=Some("+epoch+") accepted="+(pathCode===0)+" "+phase+" family_diagnostics_schema=1 family_stage_calls="+JSON.stringify(data.stage_calls)+" family_counts=[1,2,3]",at-3)]:[]),
  ...(summary?[cpu(seq,data,at-2)]:[]),
  line(seq,"output tick application sections",app+" retained="+(pathCode===1||pathCode===2)+" path_code="+pathCode,at-1),
  line(seq,"bounded output scheduler tick sample","schema_version=1 interval_micros=25000 tick_micros="+(miss?60000:14000)+" scheduled_tick=1 result_ok="+Number(pathCode!==3)+" packets="+(pathCode===3?0:16),at),
  line(seq,"bounded output scheduler deadline decision","schema_version=1 interval_micros=25000 lateness_micros="+(miss?30000:0)+" missed_deadline="+Number(miss)+" decision_code="+(miss?0:1),at+1),
  line(seq,"bounded output scheduler wait result","schema_version=1 resume_reason="+(miss?0:1),at+2),
 ].join("");
}
function cleanup(at=100,status=1,seq=null){
 return line(seq,"output pool deferred cleanup","schema_version=1 queue_lag_nanos=100 cleanup_wall_nanos=200 thread_cpu_nanos="+(status===1?150:0)+" cpu_status="+status+" counter_read_nanos=10",at);
}
const checked=text=>parseCpuDiagnostics(text,window);

test("all selected attempts and actual misses retain separate fixed CPU distributions",()=>{
 const data=source();data.stage_cpu_nanos[0]=3000;
 const r=checked(tick(1)+tick(2,{at:200,miss:true,data})+tick(3,{at:300}));
 assert.deepEqual([r.available,r.capture_complete,r.parse_complete,r.join_complete,r.cpu_complete,r.cpu_available],[1,1,1,1,1,1]);
 assert.deepEqual(r.groups.map(g=>[g.id,g.ticks,g.ordered_attempts,g.cpu_summaries]),[[0,3,3,3],[1,1,1,1]]);
 assert.ok(Math.abs(r.groups[0].stages[0].metrics[0].mean_nanos-5000/3)<1e-9);
 assert.equal(r.groups[1].stages[0].metrics[0].mean_nanos,3000);
 assert.equal(r.groups[0].workers[0].jobs,6);assert.equal(r.groups[0].workers[0].uncovered_sort_calls,3);
 assert.equal(r.groups[0].workers[0].metrics[4].max_nanos,2000);
 assert.deepEqual(validateCpuDiagnostics(JSON.parse(JSON.stringify(r))),r);
 const serialized=JSON.stringify(r);for(const value of ["2026-","tick_seq","private","generation","Some("])assert(!serialized.includes(value));
});
test("summary must match exact ordered attempt identity, call array and event order",()=>{
 const normal=tick();
 for(const mutate of [
  s=>s.replace("output_tick{tick_seq=1}: light_headless_runtime: output preparation cpu summary","output_tick{tick_seq=2}: light_headless_runtime: output preparation cpu summary"),
  s=>s.replace("2026-10-17T10:00:00.098Z","2026-10-17T10:00:00.101Z"),
  s=>s.split("\n").filter(l=>!l.includes("output preparation cpu summary")).join("\n"),
 ]){const r=checked(mutate(normal));assert.equal(r.available,0);assert.equal(r.join_complete,0);}
 const mismatch=normal.replace("family_stage_calls=[1,1,1","family_stage_calls=[2,1,1");
 assert.equal(checked(mismatch).metadata_mismatches,1);
 const duplicate=checked(normal+cpu(1));assert.equal(duplicate.selected_duplicate_records,1);assert.equal(duplicate.available,0);
});
test("retained activation may have no ordered CPU; unexpected and missing summaries fail closed",()=>{
 const retained=checked(tick(1,{pathCode:1,summary:false,nested:false})+tick(2,{at:200}));
 assert.equal(retained.available,1);assert.equal(retained.selected_ticks,2);assert.equal(retained.selected_ordered_attempts,1);assert.equal(retained.selected_cpu_summaries,1);
 const unexpected=checked(tick(1,{pathCode:1,nested:false}));assert.equal(unexpected.selected_unexpected_cpu,1);assert.equal(unexpected.available,0);
 const missing=checked(tick(1,{summary:false}));assert.equal(missing.selected_missing_cpu,1);assert.equal(missing.available,0);
 const stale=checked(tick(1,{pathCode:2}));assert.equal(stale.available,1);
});
test("unavailable and invalid CPU clocks retain statuses without fake zero CPU statistics",()=>{
 for(const status of ["unavailable","invalid"]){
  const data=source();data.stage_cpu_valid_calls[2]=0;data["stage_cpu_"+status+"_calls"][2]=1;data.stage_cpu_nanos[2]=0;
  const r=checked(tick(1,{data}));assert.equal(r.capture_complete,1);assert.equal(r.parse_complete,1);assert.equal(r.join_complete,1);
  assert.equal(r.cpu_complete,0);assert.equal(r.cpu_available,0);assert.equal(r.available,0);
  assert.equal(r.groups[0].stages[2].metrics[0].count,0);assert.equal(r.groups[0].stages[2].metrics[1].count,1);
  assert.equal(r.groups[0].stages[2][status+"_calls"],1);
 }
 const data=source();data.stage_calls[0]=2;data.stage_cpu_unavailable_calls[0]=1;
 const partial=checked(tick(1,{data}));assert.equal(partial.groups[0].stages[0].partial_cpu_records,1);assert.equal(partial.groups[0].stages[0].metrics[0].count,1);assert.equal(partial.available,0);
});
test("CPU call/job conservation and clock status inconsistencies are malformed evidence",()=>{
 const mutations=[
  d=>d.stage_cpu_valid_calls[0]=0,
  d=>{d.stage_cpu_valid_calls[0]=0;d.stage_cpu_invalid_calls[0]=1;},
  d=>d.worker_cpu_valid_jobs[0]=1,
  d=>d.worker_job_wall_max_nanos[0]=d.worker_job_wall_nanos[0]+1,
  d=>{d.worker_jobs[0]=0;d.worker_cpu_valid_jobs[0]=0;d.worker_cpu_nanos[0]=0;},
  d=>d.worker_calls[0]=0,
  d=>{d.substage_calls[0]=0;d.substage_cpu_valid_calls[0]=0;d.substage_cpu_nanos[0]=0;},
 ];
 for(const mutate of mutations){const data=source();mutate(data);const r=checked(tick(1,{data}));assert.equal(r.available,0);assert(r.invalid_records>0);}
});
test("fixed array shapes, bounded nanoseconds and unknown source fields fail closed",()=>{
 for(const mutate of [
  s=>s.replace("stage_calls=[1,1,1","stage_calls=[1,1"),
  s=>s.replace("stage_cpu_nanos=[1000","stage_cpu_nanos=[NaN"),
  s=>s.replace("stage_cpu_nanos=[1000","stage_cpu_nanos=[-1"),
  s=>s.replace("stage_cpu_nanos=[1000","stage_cpu_nanos=[1.5"),
  s=>s.replace("stage_cpu_nanos=[1000","stage_cpu_nanos=["+(CPU_LIMITS.nanos+1)),
  s=>s.replace("schema_version=1 stage_calls","schema_version=2 stage_calls"),
  s=>s.replace("schema_version=1 stage_calls","schema_version=1 private_name=secret stage_calls"),
  s=>s.replace("schema_version=1 stage_calls","schema_version=1 schema_version=1 stage_calls"),
  s=>s.replaceAll("output_tick{tick_seq=1}","wrong_span{tick_seq=1}"),
 ]){const r=checked(mutate(tick()));assert.equal(r.available,0);assert(r.invalid_records>0);assert(!JSON.stringify(r).includes("secret"));}
});
test("cleanup uses its own emitted window, never the submitting tick, and can be unbound",()=>{
 const text=tick()+cleanup(-1,1,1)+cleanup(0,1,999)+cleanup(60_000,1,null)+cleanup(60_001,1,1);
 const r=checked(text);assert.equal(r.available,1);assert.equal(r.cleanup.available,1);
 assert.deepEqual([r.cleanup.records_before_window,r.cleanup.records_in_window,r.cleanup.records_after_window],[1,2,1]);
 assert.equal(r.cleanup.valid_cpu_records,2);assert.equal(r.cleanup.metrics[2].count,2);assert.equal(r.cleanup.metrics[2].mean_nanos,150);
 assert.equal(r.groups[0].stages[0].metrics[0].mean_nanos,1000);
});
test("cleanup clock failures remain separate from attempt CPU and are never zero-filled",()=>{
 for(const status of [0,2]){const r=checked(tick()+cleanup(100,status));assert.equal(r.available,1);assert.equal(r.cpu_available,1);assert.equal(r.cleanup.available,0);assert.equal(r.cleanup.metrics[2].count,0);assert.equal(r.cleanup.metrics[0].count,1);}
 const invalid=checked(tick()+cleanup(100,0).replace("thread_cpu_nanos=0","thread_cpu_nanos=1"));
 assert.equal(invalid.parse_complete,0);assert.equal(invalid.available,0);assert.equal(invalid.cleanup.invalid_records,1);
 const badTime=checked(tick()+cleanup().replace("2026-10-17T10:00:00.100Z","no-time"));
 assert.equal(badTime.cleanup.unlocalizable_records,1);assert.equal(badTime.available,0);
});
test("miss grouping uses actual scheduler decisions rather than long wall or CPU durations",()=>{
 const r=checked(tick().replace("tick_micros=14000","tick_micros=80000"));assert.equal(r.available,1);assert.equal(r.groups[1].ticks,0);assert.equal(r.groups[1].stages[0].metrics[0].count,0);
});
test("missing or malformed base trace prevents a CPU report from claiming complete coverage",()=>{
 for(const message of ["bounded output scheduler tick sample","bounded output scheduler deadline decision","bounded output scheduler wait result","output tick application sections"]){
  const r=checked(tick().split("\n").filter(l=>!l.includes(message)).join("\n"));assert.equal(r.available,0);
 }
 const r=checked(tick().replace("tick_micros=14000","tick_micros=NaN"));assert.equal(r.available,0);assert.equal(r.selected_ticks,0);
});
test("truncated captures and process-wide tick or cleanup caps cannot be waived by the window",()=>{
 assert.equal(checked(tick().trimEnd()).read_status,8);
 for(const [message,fields,key]of [
  ["bounded output scheduler trace cap reached","schema_version=1 admitted_ticks=16384","tick_cap_records"],
  ["output pool deferred cleanup cap reached","schema_version=1 admitted_cleanups=65536","cleanup_cap_records"],
 ]){const r=checked(tick()+line(null,message,fields,61_000));assert.equal(r.read_status,9);assert.equal(r[key],1);assert.equal(r.available,0);}
 for(const [text,status]of [["x".repeat(CPU_LIMITS.bytes+1),2],["\n".repeat(CPU_LIMITS.lines+1),3],["x".repeat(CPU_LIMITS.lineBytes+1)+"\n",4]]){
  const r=checked(text);assert.equal(r.read_status,status);assert.equal(r.available,0);
 }
 assert.equal(parseCpuDiagnostics(tick(),null).read_status,7);assert.equal(checked("").available,0);
});
test("closed numeric validation rejects leaked metadata and forged counts, IDs or availability",()=>{
 const good=checked(tick()+cleanup());
 for(const mutate of [
  r=>r.pid=1,r=>r.groups[0].stages[0].symbol="secret",r=>r.groups[0].stages[0].metrics[0].count=2,
  r=>r.groups[0].stages[0].metrics[0].mean_nanos=Infinity,r=>r.groups[0].stages[0].valid_calls=0,
  r=>r.groups[0].workers[0].valid_jobs=0,r=>r.selected_cpu_summaries=0,r=>r.selected_missing_cpu=1,
  r=>r.groups[1].id=7,r=>r.groups[1].stages[0].calls=2,r=>{r.join_complete=0;r.available=0;r.cpu_available=0;},r=>r.available=0,r=>r.cleanup.metrics[2].count=0,
 ]){const bad=structuredClone(good);mutate(bad);assert.throws(()=>validateCpuDiagnostics(bad));}
});
test("bounded reads and atomic mode0600 publication retain explicit unavailable reports",async()=>{
 const dir=await mkdtemp(path.join(artifactPaths.tmp,"tl775-cpu-report-"));
 try{
  const input=path.join(dir,"private.log"),output=path.join(dir,"cpu-numeric.json");await writeFile(input,tick()+cleanup());
  const result=await writeCpuDiagnostics(output,input,window);assert.equal(result.available,1);assert.equal((await stat(output)).mode&0o777,0o600);
  assert.deepEqual(validateCpuDiagnostics(JSON.parse(await readFile(output,"utf8"))),result);
  assert.equal((await readCpuDiagnostics(path.join(dir,"missing"),window)).read_status,1);
  assert.equal((await readCpuDiagnostics(null,window)).log_available,0);
  const handle=await open(input,"w");await handle.truncate(CPU_LIMITS.bytes+1);await handle.close();assert.equal((await readCpuDiagnostics(input,window)).read_status,2);
  assert.deepEqual((await readdir(dir)).sort(),["cpu-numeric.json","private.log"]);
 }finally{await rm(dir,{recursive:true,force:true});}
});
test("metric IDs preserve fourteen stages, seven substages and three covered worker sections",()=>{
 assert.deepEqual(CPU_STAGE_IDS,PHASE_FIELDS.slice(11));assert.equal(CPU_STAGE_IDS.length,14);assert.equal(CPU_SUBSTAGE_IDS.length,7);assert.equal(CPU_WORKER_IDS.length,3);
 assert.deepEqual(CPU_ROW_METRICS,["cpu_nanos","counter_read_nanos","wall_nanos"]);
 assert.equal(CPU_WORKER_METRICS.length,5);assert.equal(CPU_CLEANUP_METRICS.length,4);
 assert.equal(CPU_LIMITS.nanos,400_000_000_000);assert.equal(CPU_LIMITS.cleanups,65536);
});

test("cleanup over-cap captures keep later reference ticks but never claim complete capture",()=>{
 const text=cleanup().repeat(CPU_LIMITS.cleanups+1)+tick();
 assert(Buffer.byteLength(text)<CPU_LIMITS.bytes);
 const report=checked(text);assert.equal(report.read_status,5);assert.equal(report.capture_complete,0);
 assert.equal(report.selected_ticks,1);assert.equal(report.selected_cpu_summaries,1);assert.equal(report.available,0);
 assert.equal(report.cleanup.records_in_window,CPU_LIMITS.cleanups);
 const capped=checked(text+line(null,"output pool deferred cleanup cap reached","schema_version=1 admitted_cleanups=65536"));
 assert.equal(capped.read_status,9);assert.equal(capped.available,0);
 const overlong=checked(text+"x".repeat(CPU_LIMITS.lineBytes+1)+"\n");
 assert.equal(overlong.read_status,4);assert.equal(overlong.available,0);
});
test("large valid reference counters cannot make malformed CPU input throw instead of failing closed",()=>{
 const text=tick().replace("packets=16","packets="+Number.MAX_SAFE_INTEGER);
 assert.equal(checked(text).selected_ticks,1);
 const report=checked(text.replace("worker_jobs=[2,2,2]","worker_jobs=[3,2,2]"));
 assert.equal(report.available,0);assert.equal(report.selected_missing_cpu,1);
});

test("closed CPU array schema exactly matches the current Rust emitter",async()=>{
 const rust=await readFile(new URL("../crates/light/adapters/headless/src/runtime/output_scheduler/slow_phase_trace/live_phases/family_preparation/cpu.rs",import.meta.url),"utf8");
 const emit=rust.slice(rust.indexOf("pub(super) fn emit(&self)"),rust.indexOf('"output preparation cpu summary"'));
 assert.deepEqual([...emit.matchAll(/\b([a-z_]+)=\?/gu)].map(m=>m[1]),Object.keys(source()));
 assert(!emit.includes("dispatch_lag"));
});
