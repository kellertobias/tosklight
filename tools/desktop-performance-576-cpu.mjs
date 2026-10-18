// Covered CPU counters and inclusive wall observations. Nested measurements overlap; never
// sum them into process CPU shares or infer worker wait from wall minus caller CPU.
// Cleanup is classified by emission time, independently of the submitting output tick.
import assert from "node:assert/strict";
import { open, mkdir, writeFile, rename, unlink } from "node:fs/promises";
import path from "node:path";
import crypto from "node:crypto";
import { diagnosticWindowBounds } from "./desktop-performance-576-clock.mjs";
import { PHASE_FIELDS } from "./desktop-performance-576-phases.mjs";
import { parseTickTraceDiagnostics, APPLICATION_FIELDS } from "./desktop-performance-576-tick-trace.mjs";

export const CPU_LIMITS = Object.freeze({bytes:32*1024*1024,lines:100_000,lineBytes:64*1024,ticks:16_384,cleanups:65_536,nanos:400_000_000_000});
export const CPU_STAGE_IDS = Object.freeze(PHASE_FIELDS.slice(11));
export const CPU_SUBSTAGE_IDS = Object.freeze(["static_discovery","assembly","prepare_current","prepare_programs","observer_finish","source_bindings","typed_drop"]);
export const CPU_WORKER_IDS = Object.freeze(["complete_deferred","prepare_families","owner_compose"]);
export const CPU_ROW_METRICS = Object.freeze(["cpu_nanos","counter_read_nanos","wall_nanos"]);
export const CPU_WORKER_METRICS = Object.freeze(["covered_cpu_nanos","counter_read_nanos","job_wall_sum_nanos","job_wall_max_nanos","section_to_job_max_nanos"]);
export const CPU_COMPLETION_KIND_IDS = Object.freeze(["ordered","indexed"]);
export const CPU_CLEANUP_METRICS = Object.freeze(["queue_lag_nanos","cleanup_wall_nanos","thread_cpu_nanos","counter_read_nanos"]);
const CPU_MESSAGE = "output preparation cpu summary";
const CLEANUP_MESSAGE = "output pool deferred cleanup";
const CLEANUP_CAP = "output pool deferred cleanup cap reached";
const TICK_CAP = "bounded output scheduler trace cap reached";
const REFERENCES = ["bounded output scheduler tick sample","bounded live output nested phase sample","bounded output scheduler deadline decision","output tick application sections"];
const STAGE_KEYS = ["stage_calls","stage_cpu_valid_calls","stage_cpu_unavailable_calls","stage_cpu_invalid_calls","stage_cpu_nanos","stage_counter_read_nanos"];
const SUB_KEYS = ["substage_calls","substage_wall_nanos","substage_cpu_valid_calls","substage_cpu_unavailable_calls","substage_cpu_invalid_calls","substage_cpu_nanos","substage_counter_read_nanos"];
const WORK_KEYS = ["worker_calls","worker_jobs","worker_cpu_valid_jobs","worker_cpu_unavailable_jobs","worker_cpu_invalid_jobs","worker_cpu_nanos","worker_counter_read_nanos","worker_job_wall_nanos","worker_job_wall_max_nanos","worker_section_to_job_max_nanos","worker_uncovered_sort_calls"];
const CPU_KEYS = ["schema_version",...STAGE_KEYS,...SUB_KEYS,...WORK_KEYS,"completion_job_kinds"];
const CLEAN_KEYS = ["schema_version","queue_lag_nanos","cleanup_wall_nanos","thread_cpu_nanos","cpu_status","counter_read_nanos"];
const FLAGS = ["available","log_available","capture_complete","parse_complete","join_complete","cpu_complete","cpu_available","tick_trace_available","tick_trace_capture_complete","tick_trace_parse_complete","tick_trace_join_complete"];
const COUNTS = ["bytes_observed","lines_observed","records_observed","cpu_records_observed","cleanup_records_observed","invalid_records","unbound_records","duplicate_records","orphan_cpu_records","tick_cap_records","cleanup_cap_records","selected_ticks","selected_missed_ticks","selected_ordered_attempts","selected_cpu_summaries","selected_missing_cpu","selected_unexpected_cpu","selected_duplicate_records","selected_invalid_records","metadata_mismatches"];
const TOP = ["schema_version",...FLAGS,"read_status","window_seconds",...COUNTS];
const STAT_KEYS = ["id","count","min_nanos","mean_nanos","p50_nanos","p95_nanos","max_nanos"];
const ROW_KEYS = ["id","calls","valid_calls","unavailable_calls","invalid_calls","called_records","cpu_records","partial_cpu_records","metrics"];
const WORKER_KEYS = ["id","calls","jobs","valid_jobs","unavailable_jobs","invalid_jobs","job_records","cpu_records","partial_cpu_records","uncovered_sort_calls","metrics"];
const COMPLETION_KEYS = ["id","jobs","valid_jobs","unavailable_jobs","invalid_jobs","metrics"];
const CLEANUP_KEYS = ["available","records_before_window","records_in_window","records_after_window","invalid_records","unlocalizable_records","valid_cpu_records","unavailable_cpu_records","invalid_cpu_records","metrics"];
const uint = n => Number.isSafeInteger(n)&&n>=0;
const countLimit = Math.floor(Number.MAX_SAFE_INTEGER/CPU_LIMITS.cleanups);
const closed = (row,keys) => assert.deepEqual(Object.keys(row).sort(),keys.slice().sort(),"closed numeric CPU shape");
const zeroStat = id => ({id,count:0,min_nanos:0,mean_nanos:0,p50_nanos:0,p95_nanos:0,max_nanos:0});
const newRow = (id,sub=false) => ({id,calls:0,valid_calls:0,unavailable_calls:0,invalid_calls:0,called_records:0,cpu_records:0,partial_cpu_records:0,metrics:Array.from({length:sub?3:2},(_,i)=>zeroStat(i))});
const newWorker = id => ({id,calls:0,jobs:0,valid_jobs:0,unavailable_jobs:0,invalid_jobs:0,job_records:0,cpu_records:0,partial_cpu_records:0,uncovered_sort_calls:0,metrics:CPU_WORKER_METRICS.map((_,i)=>zeroStat(i))});
const newCompletion = id => ({id,jobs:0,valid_jobs:0,unavailable_jobs:0,invalid_jobs:0,metrics:[zeroStat(0),zeroStat(1)]});
const newGroup = id => ({id,ticks:0,ordered_attempts:0,cpu_summaries:0,stages:CPU_STAGE_IDS.map((_,i)=>newRow(i)),substages:CPU_SUBSTAGE_IDS.map((_,i)=>newRow(i,true)),workers:CPU_WORKER_IDS.map((_,i)=>newWorker(i)),completion_kinds:[newCompletion(0),newCompletion(1)]});
const validWindow = w => diagnosticWindowBounds(w)!==null;
function blank(window,status=0) {
 return {...Object.fromEntries(TOP.map(k=>[k,0])),schema_version:2,read_status:status,window_seconds:diagnosticWindowBounds(window)?.seconds??0,groups:[newGroup(0),newGroup(1)],cleanup:{...Object.fromEntries(CLEANUP_KEYS.filter(k=>k!=="metrics").map(k=>[k,0])),metrics:CPU_CLEANUP_METRICS.map((_,i)=>zeroStat(i))}};
}
function timestamp(line) {
 const m=/^\s*(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2})(?:\.(\d{1,9}))?Z(?:\s|$)/u.exec(line);
 assert(m,"missing UTC emission time");const seconds=Date.parse(m[1]+"Z");
 assert(uint(seconds)&&new Date(seconds).toISOString().slice(0,19)===m[1]);
 return BigInt(seconds)*1_000_000n+BigInt((m[2]??"").padEnd(9,"0"));
}
function binding(line,message,required=true) {
 const at=line.indexOf(message);assert(at>=0&&line.indexOf(message,at+message.length)<0);
 const matches=[...line.slice(0,at).matchAll(/\boutput_tick\{tick_seq=(\d+)\}/gu)];
 if(!required&&matches.length===0)return null;
 assert(matches.length===1&&matches[0][1].length<=5);
 const id=BigInt(matches[0][1]);assert(id>0n&&id<=BigInt(CPU_LIMITS.ticks));return id;
}
function fields(line,message,keys) {
 const body=line.slice(line.indexOf(message)+message.length),headers=[...body.matchAll(/(?:^|\s)([A-Za-z_][A-Za-z0-9_]*)=/gu)],raw={};
 assert(headers.length===keys.length&&body.slice(0,headers[0]?.index).trim()==="");
 for(let i=0;i<headers.length;i++){const h=headers[i],k=h[1];assert(keys.includes(k)&&!Object.hasOwn(raw,k));raw[k]=body.slice(h.index+h[0].length,i+1<headers.length?headers[i+1].index:body.length).trim();}
 return raw;
}
function integer(value,max=countLimit) {assert(/^\d+$/u.test(value));const n=Number(value);assert(uint(n)&&n<=max);return n;}
function array(value,length,nanos) {
 assert(/^\[(?:\d+(?:,\s*\d+)*)?\]$/u.test(value));
 const rows=JSON.parse(value);assert(rows.length===length);
 for(const n of rows)assert(uint(n)&&n<=(nanos?CPU_LIMITS.nanos:countLimit));return rows;
}
function parseCpu(line) {
 const id=binding(line,CPU_MESSAGE),time=timestamp(line),raw=fields(line,CPU_MESSAGE,CPU_KEYS),data={};assert(integer(raw.schema_version)===2);
 for(const key of CPU_KEYS.slice(1).filter(k=>k!=="completion_job_kinds"))data[key]=array(raw[key],key.startsWith("stage_")?14:key.startsWith("substage_")?7:3,key.endsWith("_nanos"));
 data.completion_job_kinds=array(raw.completion_job_kinds,12,true);
 for(let i=0;i<2;i++){
  const v=data.completion_job_kinds.slice(i*6,i*6+6);assert(v.slice(0,4).every(n=>n<=countLimit));
  assert(v[1]+v[2]+v[3]===v[0]);if(v[1]===0)assert(v[4]===0);if(v[0]===0)assert(v[5]===0);
 }
 for(const [i,key]of ["worker_jobs","worker_cpu_valid_jobs","worker_cpu_unavailable_jobs","worker_cpu_invalid_jobs","worker_cpu_nanos","worker_counter_read_nanos"].entries())assert(data.completion_job_kinds[i]+data.completion_job_kinds[i+6]===data[key][0]);
 for(const [prefix,length,calls,valid,unavailable,invalid] of [
  ["stage",14,"calls","cpu_valid_calls","cpu_unavailable_calls","cpu_invalid_calls"],
  ["substage",7,"calls","cpu_valid_calls","cpu_unavailable_calls","cpu_invalid_calls"],
  ["worker",3,"jobs","cpu_valid_jobs","cpu_unavailable_jobs","cpu_invalid_jobs"],
 ])for(let i=0;i<length;i++){
  const get=key=>data[prefix+"_"+key][i];assert(get(valid)+get(unavailable)+get(invalid)===get(calls));
  if(get(valid)===0)assert(get("cpu_nanos")===0,"unavailable/invalid clocks cannot manufacture zero CPU observations");
  if(get(calls)===0)assert(get("counter_read_nanos")===0);
 }
 for(let i=0;i<7;i++)if(data.substage_calls[i]===0)assert(data.substage_wall_nanos[i]===0);
 for(let i=0;i<3;i++){
  assert(data.worker_job_wall_max_nanos[i]<=data.worker_job_wall_nanos[i]);
  if(data.worker_jobs[i]===0)for(const key of ["worker_job_wall_nanos","worker_job_wall_max_nanos","worker_section_to_job_max_nanos"])assert(data[key][i]===0);
  if(data.worker_calls[i]===0)assert(data.worker_jobs[i]===0);
 }
 return {id,time,data};
}
function parseCleanup(line) {
 binding(line,CLEANUP_MESSAGE,false);const time=timestamp(line),raw=fields(line,CLEANUP_MESSAGE,CLEAN_KEYS),data={};
 for(const key of CLEAN_KEYS)data[key]=integer(raw[key],key.endsWith("_nanos")?CPU_LIMITS.nanos:countLimit);
 assert(data.schema_version===1&&data.cpu_status<=2);
 if(data.cpu_status!==1)assert(data.thread_cpu_nanos===0);
 return {time,data};
}
function reference(line,kind) {
 const id=binding(line,REFERENCES[kind]),time=timestamp(line),data={};
 const expected=[
  ["schema_version","interval_micros","tick_micros","scheduled_tick","result_ok","packets"],
  [...PHASE_FIELDS,"generation","show_revision","sampled_at_millis","accepted","family_diagnostics_schema","family_stage_calls","family_counts"],
  ["schema_version","interval_micros","lateness_micros","missed_deadline","decision_code"],
  [...APPLICATION_FIELDS,"retained","path_code"],
 ][kind],raw=fields(line,REFERENCES[kind],expected);
 for(const [key,value]of Object.entries(raw)){
  if(["generation","show_revision","sampled_at_millis"].includes(key)){assert(/^(?:None|Some\(-?\d+\))$/u.test(value));continue;}
  if(key==="family_stage_calls"){data.calls=array(value,14,false);continue;}
  if(key==="family_counts"){assert(/^\[(?:\d+(?:,\s*\d+)*)?\]$/u.test(value));continue;}
  if(["accepted","retained"].includes(key)){assert(value==="true"||value==="false");continue;}
  const n=integer(value,key.endsWith("_micros")?400_000_000:Number.MAX_SAFE_INTEGER);
  if(key==="schema_version"||key==="family_diagnostics_schema")assert(n===1);
  if(["scheduled_tick","result_ok","missed_deadline","decision_code"].includes(key))assert(n<=1);
  if(key==="interval_micros")assert(n>0);if(key==="path_code")assert(n<=3);
  if(key==="missed_deadline")data.missed=n;
 }
 return {id,time,data};
}
function summarize(values,id) {
 if(!values.length)return zeroStat(id);const rows=values.slice().sort((a,b)=>a-b),at=p=>rows[Math.max(0,Math.ceil(rows.length*p)-1)];
 let mean=0;for(let i=0;i<rows.length;i++)mean+=(rows[i]-mean)/(i+1);
 return {id,count:rows.length,min_nanos:rows[0],mean_nanos:Math.min(rows.at(-1),Math.max(rows[0],mean)),p50_nanos:at(.5),p95_nanos:at(.95),max_nanos:rows.at(-1)};
}
function buckets(group) {return {stages:group.stages.map(()=>[[],[]]),substages:group.substages.map(()=>[[],[],[]]),workers:group.workers.map(()=>CPU_WORKER_METRICS.map(()=>[])),completion_kinds:group.completion_kinds.map(()=>[[],[]])};}
function addRows(rows,values,data,prefix) {
 for(const [i,row]of rows.entries()){
  const get=key=>data[prefix+"_"+key][i],calls=get("calls"),valid=get("cpu_valid_calls"),unavailable=get("cpu_unavailable_calls"),invalid=get("cpu_invalid_calls");
  row.calls+=calls;row.valid_calls+=valid;row.unavailable_calls+=unavailable;row.invalid_calls+=invalid;
  if(calls){row.called_records++;values[i][1].push(get("counter_read_nanos"));if(prefix==="substage")values[i][2].push(get("wall_nanos"));}
  if(valid){row.cpu_records++;values[i][0].push(get("cpu_nanos"));if(unavailable||invalid)row.partial_cpu_records++;}
 }
}
function addWorkers(rows,values,data) {
 for(const [i,row]of rows.entries()){
  const get=key=>data["worker_"+key][i],jobs=get("jobs"),valid=get("cpu_valid_jobs"),unavailable=get("cpu_unavailable_jobs"),invalid=get("cpu_invalid_jobs");
  row.calls+=get("calls");row.jobs+=jobs;row.valid_jobs+=valid;row.unavailable_jobs+=unavailable;row.invalid_jobs+=invalid;row.uncovered_sort_calls+=get("uncovered_sort_calls");
  if(jobs){row.job_records++;for(const [metric,key]of [[1,"counter_read_nanos"],[2,"job_wall_nanos"],[3,"job_wall_max_nanos"],[4,"section_to_job_max_nanos"]])values[i][metric].push(get(key));}
  if(valid){row.cpu_records++;values[i][0].push(get("cpu_nanos"));if(unavailable||invalid)row.partial_cpu_records++;}
 }
}
function addCompletion(rows,values,data){
 const jobs=data.worker_jobs[0],complete=data.worker_cpu_unavailable_jobs[0]===0&&data.worker_cpu_invalid_jobs[0]===0;
 for(const [i,row]of rows.entries()){
  const v=data.completion_job_kinds.slice(i*6,i*6+6);
  for(const [j,key]of ["jobs","valid_jobs","unavailable_jobs","invalid_jobs"].entries())row[key]+=v[j];
  if(jobs){values[i][1].push(v[5]);if(complete)values[i][0].push(v[4]);}
 }
}
function addGroup(group,values,entry) {
 group.ticks++;if(entry.nested)group.ordered_attempts++;if(!entry.cpu)return;
 group.cpu_summaries++;addRows(group.stages,values.stages,entry.cpu.data,"stage");addRows(group.substages,values.substages,entry.cpu.data,"substage");addWorkers(group.workers,values.workers,entry.cpu.data);addCompletion(group.completion_kinds,values.completion_kinds,entry.cpu.data);
}
function finishGroups(report,values) {
 for(const [i,group]of report.groups.entries())for(const kind of ["stages","substages","workers","completion_kinds"])for(const [j,row]of group[kind].entries())row.metrics=values[i][kind][j].map(summarize);
}
function finalize(report,entries,cleanups,window,base) {
 const {startedAtNanos:start,endedAtNanos:end}=diagnosticWindowBounds(window),values=report.groups.map(buckets);
 report.tick_trace_available=base.available;report.tick_trace_capture_complete=base.capture_complete;
 report.tick_trace_parse_complete=base.parse_complete;report.tick_trace_join_complete=base.join_complete;
 report.selected_ticks=base.selected_ticks;report.selected_missed_ticks=base.groups[1].ticks;
 const selected=[...entries.values()].filter(e=>e.tick&&e.tick.time>=start&&e.tick.time<=end);
 for(const e of selected){
  report.selected_ordered_attempts+=Number(Boolean(e.nested));report.selected_cpu_summaries+=Number(Boolean(e.cpu));
  report.selected_missing_cpu+=Number(Boolean(e.nested&&!e.cpu));report.selected_unexpected_cpu+=Number(Boolean(e.cpu&&!e.nested));
  report.selected_duplicate_records+=e.duplicates;report.selected_invalid_records+=e.invalid;
  if(e.cpu&&e.nested){
   report.metadata_mismatches+=Number(e.cpu.time<e.nested.time||e.cpu.time>e.tick.time||(e.app&&e.cpu.time>e.app.time)||JSON.stringify(e.cpu.data.stage_calls)!==JSON.stringify(e.nested.data.calls));
  }
  addGroup(report.groups[0],values[0],e);if(e.deadline?.data.missed===1)addGroup(report.groups[1],values[1],e);
 }
 report.orphan_cpu_records=[...entries.values()].filter(e=>e.cpu&&!e.tick).length;
 finishGroups(report,values);
 const cleanupValues=CPU_CLEANUP_METRICS.map(()=>[]);
 for(const {time,data}of cleanups){
  if(time<start){report.cleanup.records_before_window++;continue;}if(time>end){report.cleanup.records_after_window++;continue;}
  const c=report.cleanup;c.records_in_window++;
  if(data.cpu_status===1)c.valid_cpu_records++;else if(data.cpu_status===0)c.unavailable_cpu_records++;else c.invalid_cpu_records++;
  for(const [i,key]of CPU_CLEANUP_METRICS.entries())if(i!==2||data.cpu_status===1)cleanupValues[i].push(data[key]);
 }
 report.cleanup.metrics=cleanupValues.map(summarize);
 report.parse_complete=Number(report.capture_complete&&report.tick_trace_parse_complete&&!report.invalid_records);
 report.join_complete=Number(base.join_complete&&selected.length>0&&selected.length===report.selected_ticks&&![report.selected_missing_cpu,report.selected_unexpected_cpu,report.selected_duplicate_records,report.selected_invalid_records,report.metadata_mismatches].some(Boolean));
 const rows=[...report.groups[0].stages,...report.groups[0].substages],workers=report.groups[0].workers;
 report.cpu_complete=Number(report.selected_ticks>0&&rows.every(r=>!r.unavailable_calls&&!r.invalid_calls)&&workers.every(r=>!r.unavailable_jobs&&!r.invalid_jobs));
 report.available=Number(report.capture_complete&&report.parse_complete&&report.join_complete&&report.cpu_complete&&report.selected_ordered_attempts>0);
 report.cpu_available=Number(report.available&&(rows.some(r=>r.valid_calls>0)||workers.some(r=>r.valid_jobs>0)));
 report.cleanup.available=Number(report.capture_complete&&report.parse_complete&&report.cleanup.records_in_window>0&&!report.cleanup.unavailable_cpu_records&&!report.cleanup.invalid_cpu_records);
}
function newEntry(id){return {id,tick:null,nested:null,deadline:null,app:null,cpu:null,invalid:0,duplicates:0};}
export function parseCpuDiagnostics(text,window) {
 const report=blank(window),entries=new Map(),cleanups=[];report.log_available=1;
 if(!validWindow(window)){report.read_status=7;return validateCpuDiagnostics(report);}
 if(typeof text!=="string"||Buffer.byteLength(text)>CPU_LIMITS.bytes){report.read_status=2;return validateCpuDiagnostics(report);}
 const bytes=Buffer.from(text,"utf8");if(bytes.toString("utf8")!==text){report.log_available=0;report.read_status=6;return validateCpuDiagnostics(report);}report.bytes_observed=bytes.length;
 const lines=text.split("\n");if(lines.at(-1)==="")lines.pop();if(lines.length>CPU_LIMITS.lines){report.read_status=3;return validateCpuDiagnostics(report);}
 const base=parseTickTraceDiagnostics(text,window);
 report.lines_observed=lines.length;report.capture_complete=Number(text.length===0||text.endsWith("\n"));if(!report.capture_complete)report.read_status=8;
 const get=id=>{const key=id.toString();if(!entries.has(key))entries.set(key,newEntry(id));return entries.get(key);};
 for(const raw of lines){
  if(Buffer.byteLength(raw)>CPU_LIMITS.lineBytes){report.read_status=4;report.capture_complete=0;break;}
  const line=raw.replace(/\x1B\[[0-?]*[ -/]*[@-~]/gu,"");
  const cap=line.includes(CLEANUP_CAP)?[CLEANUP_CAP,"cleanup_cap_records","admitted_cleanups",CPU_LIMITS.cleanups]:line.includes(TICK_CAP)?[TICK_CAP,"tick_cap_records","admitted_ticks",CPU_LIMITS.ticks]:null;
  if(cap){report[cap[1]]++;try{const data=fields(line,cap[0],["schema_version",cap[2]]);assert(integer(data.schema_version)===1&&integer(data[cap[2]])===cap[3]);}catch{report.invalid_records++;}continue;}
  const kinds=[CPU_MESSAGE,CLEANUP_MESSAGE,...REFERENCES].flatMap((message,id)=>line.includes(message)?[id]:[]);if(!kinds.length)continue;
  if(kinds.length!==1){report.invalid_records++;continue;}
  const kind=kinds[0];if(kind<2){report.records_observed++;report[kind===0?"cpu_records_observed":"cleanup_records_observed"]++;}
  try{
   if(kind===1){if(cleanups.length===CPU_LIMITS.cleanups){report.read_status=5;report.capture_complete=0;continue;}cleanups.push(parseCleanup(line));continue;}
   const parsed=kind===0?parseCpu(line):reference(line,kind-2),entry=get(parsed.id),key=kind===0?"cpu":["tick","nested","deadline","app"][kind-2];
   if(entry[key]){entry.duplicates++;report.duplicate_records++;}else entry[key]=parsed;
  }catch{
   report.invalid_records++;
   if(kind===1){report.cleanup.invalid_records++;try{timestamp(line);}catch{report.cleanup.unlocalizable_records++;}}
   else{try{get(binding(line,kind===0?CPU_MESSAGE:REFERENCES[kind-2])).invalid++;}catch{report.unbound_records++;}}
  }
 }
 if(report.tick_cap_records||report.cleanup_cap_records){report.read_status=9;report.capture_complete=0;}
 if(!base.capture_complete&&report.capture_complete){report.capture_complete=0;report.read_status=base.read_status;}
 finalize(report,entries,cleanups,window,base);return validateCpuDiagnostics(report);
}
function validateStat(stat,id,maxCount) {
 closed(stat,STAT_KEYS);assert(stat.id===id&&uint(stat.count)&&stat.count<=maxCount);
 for(const key of STAT_KEYS.slice(2))assert(typeof stat[key]==="number"&&Number.isFinite(stat[key])&&stat[key]>=0&&stat[key]<=CPU_LIMITS.nanos);
 assert(stat.min_nanos<=stat.p50_nanos&&stat.p50_nanos<=stat.p95_nanos&&stat.p95_nanos<=stat.max_nanos&&stat.min_nanos<=stat.mean_nanos&&stat.mean_nanos<=stat.max_nanos);
 for(const key of ["min_nanos","p50_nanos","p95_nanos","max_nanos"])assert(uint(stat[key]));
 if(!stat.count)assert(STAT_KEYS.slice(2).every(key=>stat[key]===0));
}
function validateRow(row,id,group,sub) {
 closed(row,ROW_KEYS);assert(row.id===id);
 for(const key of ROW_KEYS.filter(k=>!["metrics","id"].includes(k)))assert(uint(row[key]));
 assert(row.valid_calls+row.unavailable_calls+row.invalid_calls===row.calls&&row.called_records<=group.cpu_summaries&&row.cpu_records<=row.called_records&&row.partial_cpu_records<=row.cpu_records);
 assert(row.cpu_records<=row.valid_calls&&row.called_records<=row.calls);
 assert(Boolean(row.cpu_records)===Boolean(row.valid_calls)&&Boolean(row.called_records)===Boolean(row.calls));
 if(!row.unavailable_calls&&!row.invalid_calls)assert(row.partial_cpu_records===0);
 assert(row.metrics.length===(sub?3:2));row.metrics.forEach((s,i)=>validateStat(s,i,group.cpu_summaries));
 assert(row.metrics[0].count===row.cpu_records&&row.metrics[1].count===row.called_records);if(sub)assert(row.metrics[2].count===row.called_records);
}
function validateWorker(row,id,group) {
 closed(row,WORKER_KEYS);assert(row.id===id);for(const key of WORKER_KEYS.filter(k=>!["id","metrics"].includes(k)))assert(uint(row[key]));
 assert(row.valid_jobs+row.unavailable_jobs+row.invalid_jobs===row.jobs&&row.job_records<=group.cpu_summaries&&row.job_records<=row.jobs&&row.cpu_records<=row.job_records&&row.cpu_records<=row.valid_jobs&&row.partial_cpu_records<=row.cpu_records);
 assert(Boolean(row.cpu_records)===Boolean(row.valid_jobs)&&Boolean(row.job_records)===Boolean(row.jobs));
 if(!row.unavailable_jobs&&!row.invalid_jobs)assert(row.partial_cpu_records===0);
 assert(row.metrics.length===5);row.metrics.forEach((s,i)=>validateStat(s,i,group.cpu_summaries));assert(row.metrics[0].count===row.cpu_records);for(let i=1;i<5;i++)assert(row.metrics[i].count===row.job_records);
}
function validateCompletion(group){
 const rows=group.completion_kinds,parent=group.workers[0];assert(Array.isArray(rows)&&rows.length===2);
 for(const [id,row]of rows.entries()){
  closed(row,COMPLETION_KEYS);assert(row.id===id&&["jobs","valid_jobs","unavailable_jobs","invalid_jobs"].every(k=>uint(row[k])));
  assert(row.valid_jobs+row.unavailable_jobs+row.invalid_jobs===row.jobs&&row.metrics.length===2);row.metrics.forEach((s,i)=>validateStat(s,i,group.cpu_summaries));
  if(row.valid_jobs===0)assert(row.metrics[0].max_nanos===0);if(row.jobs===0)assert(row.metrics[1].max_nanos===0);
  assert(row.metrics[1].count===parent.job_records&&row.metrics[0].count===parent.cpu_records-parent.partial_cpu_records);
 }
 for(const key of ["jobs","valid_jobs","unavailable_jobs","invalid_jobs"])assert(rows[0][key]+rows[1][key]===parent[key]);
 for(let i=0;i<2;i++){
  const a=rows[0].metrics[i],b=rows[1].metrics[i],total=parent.metrics[i];assert(a.count===b.count);
  if(a.count&&a.count===total.count){const tolerance=32*Number.EPSILON*a.count*Math.max(1,total.max_nanos,a.max_nanos,b.max_nanos);assert(Math.abs(a.mean_nanos+b.mean_nanos-total.mean_nanos)<=tolerance);}
 }
}
function validateSubset(all,missed) {
 for(const key of ["ticks","ordered_attempts","cpu_summaries"])assert(missed[key]<=all[key]);
 for(const kind of ["stages","substages","workers","completion_kinds"])for(const [i,row]of missed[kind].entries()){
  const parent=all[kind][i];
  for(const key of Object.keys(row).filter(k=>!["id","metrics"].includes(k)))assert(row[key]<=parent[key]);
  for(const [j,metric]of row.metrics.entries()){
   const full=parent.metrics[j];assert(metric.count<=full.count);
   if(metric.count)assert(metric.min_nanos>=full.min_nanos&&metric.max_nanos<=full.max_nanos);
   if(metric.count===full.count)assert.deepEqual(metric,full);
  }
 }
}
export function validateCpuDiagnostics(report) {
 closed(report,[...TOP,"groups","cleanup"]);for(const key of TOP)assert(typeof report[key]==="number"&&Number.isFinite(report[key])&&report[key]>=0);
 assert(report.schema_version===2&&uint(report.read_status)&&report.read_status<=9&&report.window_seconds<=400);
 for(const key of FLAGS)assert(report[key]===0||report[key]===1);for(const key of COUNTS)assert(uint(report[key]));
 assert(report.capture_complete<=report.log_available&&report.capture_complete===Number(report.read_status===0));
 if(report.read_status===7)assert(report.window_seconds===0);else assert(report.window_seconds>0);
 assert(report.parse_complete===Number(Boolean(report.capture_complete&&report.tick_trace_parse_complete&&!report.invalid_records)));
 assert(report.join_complete===Number(Boolean(report.tick_trace_join_complete&&report.selected_ticks>0&&![report.selected_missing_cpu,report.selected_unexpected_cpu,report.selected_duplicate_records,report.selected_invalid_records,report.metadata_mismatches].some(Boolean))));
 assert(report.tick_trace_available===Number(Boolean(report.tick_trace_capture_complete&&report.tick_trace_parse_complete&&report.tick_trace_join_complete&&report.selected_ticks>0)));
 if([1,2,6,7].includes(report.read_status))assert(report.bytes_observed===0);if(report.capture_complete&&report.lines_observed)assert(report.bytes_observed>0);
 assert(report.bytes_observed<=CPU_LIMITS.bytes&&report.records_observed===report.cpu_records_observed+report.cleanup_records_observed&&report.lines_observed<=CPU_LIMITS.lines);
 assert(report.selected_ticks<=CPU_LIMITS.ticks&&report.selected_ordered_attempts<=report.selected_ticks&&report.selected_cpu_summaries<=report.selected_ticks);
 assert(report.selected_missing_cpu<=report.selected_ordered_attempts&&report.selected_unexpected_cpu<=report.selected_cpu_summaries&&report.selected_invalid_records<=report.invalid_records&&report.selected_duplicate_records<=report.duplicate_records);
 if(report.tick_cap_records||report.cleanup_cap_records)assert(report.read_status===9&&report.capture_complete===0);if(report.read_status===9)assert(report.tick_cap_records+report.cleanup_cap_records>0);
 assert(Array.isArray(report.groups)&&report.groups.length===2);
 for(const [id,g]of report.groups.entries()){
  closed(g,["id","ticks","ordered_attempts","cpu_summaries","stages","substages","workers","completion_kinds"]);assert(g.id===id);
  for(const key of ["ticks","ordered_attempts","cpu_summaries"])assert(uint(g[key])&&g[key]<=report.selected_ticks);
  assert(g.ordered_attempts<=g.ticks&&g.cpu_summaries<=g.ticks&&g.stages.length===14&&g.substages.length===7&&g.workers.length===3);
  g.stages.forEach((r,i)=>validateRow(r,i,g,false));g.substages.forEach((r,i)=>validateRow(r,i,g,true));g.workers.forEach((r,i)=>validateWorker(r,i,g));validateCompletion(g);
 }
 validateSubset(report.groups[0],report.groups[1]);
 assert(report.selected_ordered_attempts-report.selected_missing_cpu+report.selected_unexpected_cpu===report.selected_cpu_summaries);
 assert(report.groups[1].ticks===report.selected_missed_ticks);
 assert(report.groups[0].ticks===report.selected_ticks&&report.groups[0].ordered_attempts===report.selected_ordered_attempts&&report.groups[0].cpu_summaries===report.selected_cpu_summaries);
 const rows=[...report.groups[0].stages,...report.groups[0].substages],workers=report.groups[0].workers;
 const complete=rows.every(r=>!r.unavailable_calls&&!r.invalid_calls)&&workers.every(r=>!r.unavailable_jobs&&!r.invalid_jobs);
 assert(report.cpu_complete===Number(report.selected_ticks>0&&complete));
 assert(report.available===Number(Boolean(report.capture_complete&&report.parse_complete&&report.join_complete&&report.cpu_complete&&report.selected_ordered_attempts>0)));
 assert(report.cpu_available===Number(Boolean(report.available&&(rows.some(r=>r.valid_calls>0)||workers.some(r=>r.valid_jobs>0)))));
 if(report.join_complete)assert(report.selected_ticks>0&&report.selected_ordered_attempts===report.selected_cpu_summaries&&![report.selected_missing_cpu,report.selected_unexpected_cpu,report.selected_duplicate_records,report.selected_invalid_records,report.metadata_mismatches].some(Boolean));
 if(report.invalid_records)assert(report.parse_complete===0);
 const c=report.cleanup;closed(c,CLEANUP_KEYS);for(const key of CLEANUP_KEYS.filter(k=>k!=="metrics"))assert(uint(c[key]));
 assert(c.available<=1&&c.records_before_window+c.records_in_window+c.records_after_window<=CPU_LIMITS.cleanups);
 assert(c.valid_cpu_records+c.unavailable_cpu_records+c.invalid_cpu_records===c.records_in_window&&c.invalid_records<=report.invalid_records&&c.unlocalizable_records<=c.invalid_records);
 assert(c.records_before_window+c.records_in_window+c.records_after_window+c.invalid_records<=report.cleanup_records_observed);
 if(report.capture_complete)assert(c.records_before_window+c.records_in_window+c.records_after_window+c.invalid_records===report.cleanup_records_observed);
 assert(c.metrics.length===4);c.metrics.forEach((s,i)=>validateStat(s,i,c.records_in_window));
 for(const i of [0,1,3])assert(c.metrics[i].count===c.records_in_window);assert(c.metrics[2].count===c.valid_cpu_records);
 assert(c.available===Number(Boolean(report.capture_complete&&report.parse_complete&&c.records_in_window>0&&!c.unavailable_cpu_records&&!c.invalid_cpu_records)));
 return report;
}
export async function readCpuDiagnostics(file,window) {
 if(!validWindow(window))return validateCpuDiagnostics(blank(null,7));
 if(!file)return validateCpuDiagnostics(blank(window,1));
 let handle,text,status=0;
 try{
  handle=await open(file,"r");if((await handle.stat()).size>CPU_LIMITS.bytes)status=2;
  else{const buffer=Buffer.alloc(CPU_LIMITS.bytes+1);let used=0;while(used<buffer.length){const result=await handle.read(buffer,used,buffer.length-used,null);if(!result.bytesRead)break;used+=result.bytesRead;}if(used>CPU_LIMITS.bytes)status=2;else text=new TextDecoder("utf-8",{fatal:true,ignoreBOM:true}).decode(buffer.subarray(0,used));}
 }catch(error){status=error.code==="ENOENT"?1:6;}finally{await handle?.close();}
 if(status){const r=blank(window,status);if(status===2)r.log_available=1;return validateCpuDiagnostics(r);}
 return parseCpuDiagnostics(text,window);
}
export async function writeCpuDiagnostics(destination,file,window) {
 const report=await readCpuDiagnostics(file,window);await mkdir(path.dirname(destination),{recursive:true});const temporary=destination+"."+crypto.randomUUID()+".partial";
 try{await writeFile(temporary,JSON.stringify(report,null,2)+"\n",{flag:"wx",mode:0o600});await rename(temporary,destination);}finally{await unlink(temporary).catch(()=>{});}return report;
}
