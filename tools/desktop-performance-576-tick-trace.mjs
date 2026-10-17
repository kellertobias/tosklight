// Diagnostic wall-time trace only. Raw lines, timestamps, tick identities and paths stay private.
import assert from "node:assert/strict";
import { open, mkdir, writeFile, rename, unlink } from "node:fs/promises";
import path from "node:path";
import crypto from "node:crypto";
import { PHASE_FIELDS } from "./desktop-performance-576-phases.mjs";

export const TICK_TRACE_LIMITS = Object.freeze({bytes:32*1024*1024,lines:100_000,lineBytes:64*1024,ticks:16_384,micros:400_000_000});
export const APPLICATION_FIELDS = Object.freeze(["application_total_micros","ordered_operation_micros","audio_reconcile_micros","publication_payload_micros","delivery_micros"]);
export const TICK_METRIC_FIELDS = Object.freeze([...PHASE_FIELDS,...APPLICATION_FIELDS,"tick_micros","interval_micros","lateness_micros"]);
const MESSAGES = ["bounded output scheduler tick sample","bounded output scheduler deadline decision","bounded output scheduler wait result","output tick application sections","bounded live output nested phase sample"];
const CAP = "bounded output scheduler trace cap reached";
const FIELDS = [
 ["schema_version","interval_micros","tick_micros","scheduled_tick","result_ok","packets"],
 ["schema_version","interval_micros","lateness_micros","missed_deadline","decision_code"],
 ["schema_version","resume_reason"],
 [...APPLICATION_FIELDS,"retained","path_code"],
 [...PHASE_FIELDS,"generation","show_revision","sampled_at_millis","accepted","family_diagnostics_schema","family_stage_calls","family_counts"],
];
const COUNTS = ["lines_observed","records_observed","ambiguous_records","invalid_records","unbound_records","duplicate_records","orphan_records","ticks_observed","ticks_before_window","selected_ticks","ticks_after_window","selected_tick_seq_span","selected_missing_tick","selected_missing_deadline","selected_missing_wait","selected_missing_application","selected_missing_nested","selected_duplicate_records","selected_invalid_records","metadata_mismatches","sequence_range_errors","nested_accepted","nested_rejected","retained_ticks","error_ticks","tick_cap_records"];
const FLAGS = ["available","log_available","capture_complete","parse_complete","join_complete","health_deadline_available","completed_frames_available","health_deadline_comparison_available","completed_frames_comparison_available"];
const TOP = ["schema_version",...FLAGS,"read_status","window_seconds",...COUNTS,"health_deadline_misses","health_deadline_delta","completed_frames","successful_ticks","completed_frame_delta"];
const STAT = ["id","count","min_micros","mean_micros","p50_micros","p95_micros","max_micros"];
const UINT = value => Number.isSafeInteger(value) && value >= 0;
const bounded = value => UINT(value) && value <= TICK_TRACE_LIMITS.micros;
const keys = (value,expected) => assert.deepEqual(Object.keys(value).sort(),expected.slice().sort(),"closed numeric trace shape");
const zeroMetric = id => ({id,count:0,min_micros:0,mean_micros:0,p50_micros:0,p95_micros:0,max_micros:0});
const emptyGroup = id => ({id,ticks:0,nested_ticks:0,nested_accepted:0,nested_rejected:0,metrics:TICK_METRIC_FIELDS.map((_,id)=>zeroMetric(id))});
const validWindow = w => Number.isFinite(w?.startedAt) && Number.isFinite(w?.endedAt) && w.startedAt >= 0 && w.endedAt > w.startedAt && w.endedAt-w.startedAt <= 400_000;
const nanos = n => BigInt(Math.trunc(n))*1_000_000n+BigInt(Math.round((n-Math.trunc(n))*1_000_000));
function blank(window,status=0,observed={}) {
 const r={...Object.fromEntries(TOP.map(k=>[k,0])),schema_version:1,read_status:status,window_seconds:validWindow(window)?(window.endedAt-window.startedAt)/1000:0,groups:[emptyGroup(0),emptyGroup(1)],record_kinds:MESSAGES.map((_,id)=>({id,count:0})),paths:Array.from({length:4},(_,id)=>({id,count:0}))};
 for(const [input,flag,field] of [["healthDeadlineMisses","health_deadline_available","health_deadline_misses"],["completedFrames","completed_frames_available","completed_frames"]]) {
  if(observed[input] !== undefined && observed[input] !== null) {assert(UINT(observed[input]));r[flag]=1;r[field]=observed[input];}
 }
 return r;
}
function timestamp(line) {
 const m=/^\s*(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2})(?:\.(\d{1,9}))?Z(?:\s|$)/u.exec(line);
 if(!m)return null;const seconds=Date.parse(`${m[1]}Z`);
 if(!UINT(seconds)||new Date(seconds).toISOString().slice(0,19)!==m[1])return null;
 return BigInt(seconds)*1_000_000n+BigInt((m[2]??"").padEnd(9,"0"));
}
function fields(body,expected) {
 const headers=[...body.matchAll(/(?:^|\s)([A-Za-z_][A-Za-z0-9_]*)=/gu)],result={};
 assert(headers.length===expected.length&&body.slice(0,headers[0]?.index).trim()==="");
 for(let i=0;i<headers.length;i++) {
  const h=headers[i],name=h[1];assert(expected.includes(name)&&!Object.hasOwn(result,name));
  result[name]=body.slice(h.index+h[0].length,i+1<headers.length?headers[i+1].index:body.length).trim();
 }
 return result;
}
function integer(text,max=Number.MAX_SAFE_INTEGER) {
 assert(/^\d+$/u.test(text??""));const n=Number(text);assert(UINT(n)&&n<=max);return n;
}
function parseRecord(line,kind) {
 // Span fields are parsed separately, before the event payload: no guessed global field scan.
 const message=MESSAGES[kind],at=line.indexOf(message);assert(at>=0&&line.indexOf(message,at+message.length)<0);
 const spans=[...line.slice(0,at).matchAll(/\boutput_tick\{tick_seq=(\d+)\}/gu)];assert(spans.length===1);
 assert(spans[0][1].length<=5);const seq=BigInt(spans[0][1]);assert(seq>0n&&seq<=BigInt(TICK_TRACE_LIMITS.ticks));
 const time=timestamp(line);assert(time!==null);const raw=fields(line.slice(at+message.length),FIELDS[kind]),data={};
 for(const [name,value] of Object.entries(raw)) {
  if(kind===4&&["generation","show_revision","sampled_at_millis"].includes(name)) {assert(/^(?:None|Some\(-?\d+\))$/u.test(value));continue;}
  if(kind===4&&["family_stage_calls","family_counts"].includes(name)) {assert(/^\[(?:\d+(?:,\s*\d+)*)?\]$/u.test(value));continue;}
  if(["accepted","retained"].includes(name)) {assert(["true","false"].includes(value));data[name]=Number(value==="true");continue;}
  data[name]=integer(value,name.endsWith("_micros")?TICK_TRACE_LIMITS.micros:Number.MAX_SAFE_INTEGER);
 }
 if(kind<3)assert(data.schema_version===1);
 if(kind===4)assert(data.family_diagnostics_schema===1);
 if(kind===0)assert(data.interval_micros>0&&[0,1].includes(data.scheduled_tick)&&[0,1].includes(data.result_ok));
 if(kind===1)assert(data.interval_micros>0&&[0,1].includes(data.missed_deadline)&&[0,1].includes(data.decision_code));
 if(kind===2)assert(data.resume_reason<=3);
 if(kind===3)assert(data.path_code<=3);
 return {seq,time,data};
}
function summarize(values,id) {
 if(!values.length)return zeroMetric(id);const xs=values.slice().sort((a,b)=>a-b),at=p=>xs[Math.max(0,Math.ceil(xs.length*p)-1)];
 return {id,count:xs.length,min_micros:xs[0],mean_micros:xs.reduce((a,b)=>a+b,0)/xs.length,p50_micros:at(.5),p95_micros:at(.95),max_micros:xs.at(-1)};
}
function mismatch(entry) {
 const [tick,deadline,wait,app,nested]=entry.parts.map(p=>p?.data);if(!tick||!deadline||!wait||!app)return 0;
 let bad=tick.interval_micros!==deadline.interval_micros;
 bad ||= deadline.decision_code===1&&(deadline.lateness_micros!==0||deadline.missed_deadline!==0||wait.resume_reason===0);
 bad ||= deadline.decision_code===0&&wait.resume_reason!==0;
 bad ||= deadline.missed_deadline===1&&deadline.lateness_micros<deadline.interval_micros;
 bad ||= deadline.missed_deadline===0&&deadline.lateness_micros>deadline.interval_micros;
 bad ||= app.path_code!==3&&app.retained!==Number(app.path_code===1||app.path_code===2);
 bad ||= tick.result_ok!==Number(app.path_code!==3)||(tick.result_ok===0&&tick.packets!==0);
 if(nested)bad ||= (app.path_code===0&&nested.accepted!==1)||(app.path_code===2&&nested.accepted!==0)||app.path_code===1||(app.path_code===3&&app.retained&&nested.accepted!==0);
 const [tickRecord,decisionRecord,waitRecord,appRecord,nestedRecord]=entry.parts;
 bad ||= appRecord.time>tickRecord.time||decisionRecord.time<tickRecord.time||waitRecord.time<decisionRecord.time||(nestedRecord&&nestedRecord.time>appRecord.time);
 return Number(bad);
}
function summarizeEntries(report,entries,window) {
 const first=nanos(window.startedAt),last=nanos(window.endedAt),all=[...entries.values()],selected=all.filter(e=>e.parts[0]&&e.parts[0].time>=first&&e.parts[0].time<=last).sort((a,b)=>a.seq<b.seq?-1:1);
 report.ticks_observed=all.filter(e=>e.parts[0]).length;report.selected_ticks=selected.length;
 report.ticks_before_window=all.filter(e=>e.parts[0]&&e.parts[0].time<first).length;report.ticks_after_window=report.ticks_observed-report.ticks_before_window-selected.length;
 report.orphan_records=all.filter(e=>!e.parts[0]).reduce((n,e)=>n+e.parts.filter(Boolean).length,0);
 if(!selected.length)return;
 const low=selected[0].seq,high=selected.at(-1).seq,span=high-low+1n;
 report.sequence_range_errors=Number(span>BigInt(TICK_TRACE_LIMITS.ticks));
 if(!report.sequence_range_errors){report.selected_tick_seq_span=Number(span);report.selected_missing_tick=Number(span)-selected.length;}
 for(const e of all.filter(e=>e.seq>=low&&e.seq<=high)) {report.selected_duplicate_records+=e.duplicates;report.selected_invalid_records+=e.invalid;}
 const values=[TICK_METRIC_FIELDS.map(()=>[]),TICK_METRIC_FIELDS.map(()=>[])];
 for(const e of selected) {
  const [tick,deadline,wait,app,nested]=e.parts.map(p=>p?.data);
  if(!deadline)report.selected_missing_deadline++;if(!wait)report.selected_missing_wait++;if(!app)report.selected_missing_application++;
  if(app&&!nested&&(app.path_code===0||app.path_code===2||(app.path_code===3&&!app.retained)))report.selected_missing_nested++;
  report.metadata_mismatches+=mismatch(e);
  if(app){report.paths[app.path_code].count++;report.retained_ticks+=app.retained;report.error_ticks+=Number(app.path_code===3);}
  if(nested){report.nested_accepted+=nested.accepted;report.nested_rejected+=1-nested.accepted;}
  report.successful_ticks+=tick.result_ok;
  for(const group of [0,...(deadline?.missed_deadline===1?[1]:[])]) {
   const g=report.groups[group];g.ticks++;if(nested){g.nested_ticks++;g.nested_accepted+=nested.accepted;g.nested_rejected+=1-nested.accepted;}
   for(const [id,name] of TICK_METRIC_FIELDS.entries()) {const value=id<25?nested?.[name]:id<30?app?.[name]:name==="lateness_micros"?deadline?.[name]:tick[name];if(value!==undefined)values[group][id].push(value);}
  }
 }
 report.groups.forEach((g,i)=>{g.metrics=values[i].map(summarize);});
 report.join_complete=Number(![report.selected_missing_tick,report.selected_missing_deadline,report.selected_missing_wait,report.selected_missing_application,report.selected_missing_nested,report.selected_duplicate_records,report.selected_invalid_records,report.metadata_mismatches,report.sequence_range_errors].some(Boolean));
}

export function parseTickTraceDiagnostics(text,window,observed={}) {
 const report=blank(window,0,observed),entries=new Map();report.log_available=1;
 if(!validWindow(window)){report.read_status=7;return validateTickTraceDiagnostics(report);}
 if(typeof text!=="string"||Buffer.byteLength(text)>TICK_TRACE_LIMITS.bytes){report.read_status=2;return validateTickTraceDiagnostics(report);}
 const lines=text.split("\n");if(lines.at(-1)==="")lines.pop();
 if(lines.length>TICK_TRACE_LIMITS.lines){report.read_status=3;return validateTickTraceDiagnostics(report);}
 report.lines_observed=lines.length;report.capture_complete=Number(text.length===0||text.endsWith("\n"));if(!report.capture_complete)report.read_status=8;
 for(const raw of lines) {
  if(Buffer.byteLength(raw)>TICK_TRACE_LIMITS.lineBytes){report.read_status=4;report.capture_complete=0;break;}
  const line=raw.replace(/\x1B\[[0-?]*[ -/]*[@-~]/gu,"");
  if(line.includes(CAP)) {report.tick_cap_records++;try{const cap=fields(line.slice(line.indexOf(CAP)+CAP.length),["schema_version","admitted_ticks"]);assert(cap.schema_version==="1"&&cap.admitted_ticks==="16384");}catch{report.invalid_records++;}continue;}
  const kinds=MESSAGES.flatMap((m,i)=>line.includes(m)?[i]:[]);if(!kinds.length)continue;
  report.records_observed++;if(kinds.length!==1){report.ambiguous_records++;report.invalid_records++;report.unbound_records++;continue;}
  const kind=kinds[0];report.record_kinds[kind].count++;
  try {
   const record=parseRecord(line,kind),key=record.seq.toString();
   if(!entries.has(key)){if(entries.size===TICK_TRACE_LIMITS.ticks){report.read_status=5;report.capture_complete=0;break;}entries.set(key,{seq:record.seq,parts:Array(5).fill(null),duplicates:0,invalid:0});}
   const entry=entries.get(key);if(entry.parts[kind]){entry.duplicates++;report.duplicate_records++;}else entry.parts[kind]=record;
  } catch {
   report.invalid_records++;const seqs=[...line.matchAll(/\boutput_tick\{tick_seq=(\d+)\}/gu)];
   if(seqs.length===1&&seqs[0][1].length<=5&&BigInt(seqs[0][1])>0n&&BigInt(seqs[0][1])<=BigInt(TICK_TRACE_LIMITS.ticks)) {
    const key=BigInt(seqs[0][1]).toString();if(!entries.has(key))entries.set(key,{seq:BigInt(key),parts:Array(5).fill(null),duplicates:0,invalid:0});entries.get(key).invalid++;
   }else report.unbound_records++;
  }
 }
 if(report.tick_cap_records){report.read_status=9;report.capture_complete=0;}
 report.parse_complete=Number(report.capture_complete&&!report.invalid_records);
 summarizeEntries(report,entries,window);report.available=Number(report.capture_complete&&report.parse_complete&&report.join_complete&&report.selected_ticks>0);
 report.health_deadline_comparison_available=Number(Boolean(report.available&&report.health_deadline_available));
 report.completed_frames_comparison_available=Number(Boolean(report.available&&report.completed_frames_available));
 if(report.health_deadline_comparison_available)report.health_deadline_delta=report.groups[1].ticks-report.health_deadline_misses;
 if(report.completed_frames_comparison_available)report.completed_frame_delta=report.successful_ticks-report.completed_frames;
 return validateTickTraceDiagnostics(report);
}

export function validateTickTraceDiagnostics(report) {
 keys(report,[...TOP,"groups","record_kinds","paths"]);
 for(const name of TOP)assert(typeof report[name]==="number"&&Number.isFinite(report[name])&&(name.endsWith("_delta")?Number.isSafeInteger(report[name]):report[name]>=0),"finite numeric trace field");
 assert(report.schema_version===1&&UINT(report.read_status)&&report.read_status<=9&&report.window_seconds<=400);
 for(const name of FLAGS)assert([0,1].includes(report[name]));for(const name of [...COUNTS,"health_deadline_misses","completed_frames","successful_ticks"])assert(UINT(report[name]));
 assert(report.capture_complete<=report.log_available&&report.capture_complete===Number(report.read_status===0));
 if(report.read_status===7)assert(report.window_seconds===0);else assert(report.window_seconds>0);
 assert(report.parse_complete===Number(Boolean(report.capture_complete&&!report.invalid_records)));
 assert(report.available===Number(Boolean(report.capture_complete&&report.parse_complete&&report.join_complete&&report.selected_ticks)));
 if(report.tick_cap_records)assert(report.read_status===9&&report.capture_complete===0);
 if(report.read_status===9)assert(report.tick_cap_records>0);
 assert(report.lines_observed<=TICK_TRACE_LIMITS.lines&&report.records_observed<=report.lines_observed);
 assert(report.unbound_records<=report.invalid_records&&report.selected_invalid_records<=report.invalid_records&&report.selected_duplicate_records<=report.duplicate_records);
 assert(report.ticks_observed===report.ticks_before_window+report.selected_ticks+report.ticks_after_window);
 if(!report.sequence_range_errors)assert(report.selected_tick_seq_span===report.selected_ticks+report.selected_missing_tick);else assert(report.selected_tick_seq_span===0&&report.selected_missing_tick===0);
 assert(report.selected_ticks<=TICK_TRACE_LIMITS.ticks&&report.records_observed<=TICK_TRACE_LIMITS.lines);
 assert(report.groups.length===2&&report.record_kinds.length===5&&report.paths.length===4);
 for(const [index,group] of report.groups.entries()) {
  keys(group,["id","ticks","nested_ticks","nested_accepted","nested_rejected","metrics"]);assert(group.id===index);
  for(const name of ["ticks","nested_ticks","nested_accepted","nested_rejected"])assert(UINT(group[name]));
  assert(group.ticks<=report.selected_ticks&&group.nested_ticks<=group.ticks&&group.nested_accepted+group.nested_rejected===group.nested_ticks);
  assert(group.metrics.length===33);
  for(const [id,m] of group.metrics.entries()) {keys(m,STAT);assert(m.id===id&&UINT(m.count)&&m.count<=group.ticks);for(const [name,value] of Object.entries(m))assert(typeof value==="number"&&Number.isFinite(value)&&value>=0);assert(m.min_micros<=m.p50_micros&&m.p50_micros<=m.p95_micros&&m.p95_micros<=m.max_micros&&m.min_micros<=m.mean_micros&&m.mean_micros<=m.max_micros&&m.max_micros<=TICK_TRACE_LIMITS.micros);if(!m.count)assert(Object.values(m).slice(1).every(v=>v===0));for(const name of ["min_micros","p50_micros","p95_micros","max_micros"])assert(UINT(m[name]));}
 }
 assert(report.groups[0].ticks===report.selected_ticks&&report.groups[1].ticks<=report.selected_ticks);
 for(const group of report.groups) {
  for(let id=0;id<25;id++)assert(group.metrics[id].count===group.nested_ticks);
  for(let id=25;id<30;id++)assert(group.metrics[id].count===group.metrics[25].count);
  assert(group.metrics[30].count===group.ticks&&group.metrics[31].count===group.ticks);
 }
 assert(report.groups[0].metrics[25].count===report.selected_ticks-report.selected_missing_application);
 assert(report.groups[0].metrics[32].count===report.selected_ticks-report.selected_missing_deadline);
 assert(report.groups[1].metrics[32].count===report.groups[1].ticks);
 assert(report.record_kinds.reduce((n,row)=>n+row.count,0)+report.ambiguous_records===report.records_observed);
 assert(report.successful_ticks<=report.selected_ticks&&report.retained_ticks<=report.selected_ticks&&report.error_ticks===report.paths[3].count);
 if(report.join_complete) {
  const retainedPaths=report.paths[1].count+report.paths[2].count;assert(report.retained_ticks>=retainedPaths&&report.retained_ticks<=retainedPaths+report.error_ticks);
  assert(report.successful_ticks+report.error_ticks===report.selected_ticks);
  for(const group of report.groups)for(const [id,metric] of group.metrics.entries())assert(metric.count===(id<25?group.nested_ticks:group.ticks));
 }
 assert(report.nested_accepted===report.groups[0].nested_accepted&&report.nested_rejected===report.groups[0].nested_rejected);
 for(const [rows,length] of [[report.record_kinds,5],[report.paths,4]]) {assert(rows.length===length);for(const [id,row] of rows.entries()){keys(row,["id","count"]);assert(row.id===id&&UINT(row.count));}}
 assert(report.paths.reduce((n,p)=>n+p.count,0)+report.selected_missing_application===report.selected_ticks);
 assert(report.health_deadline_comparison_available===Number(Boolean(report.available&&report.health_deadline_available)));
 if(report.health_deadline_comparison_available)assert(report.health_deadline_delta===report.groups[1].ticks-report.health_deadline_misses);else assert(report.health_deadline_delta===0);
 if(!report.health_deadline_available)assert(report.health_deadline_misses===0);
 assert(report.completed_frames_comparison_available===Number(Boolean(report.available&&report.completed_frames_available)));
 if(report.completed_frames_comparison_available)assert(report.completed_frame_delta===report.successful_ticks-report.completed_frames);else assert(report.completed_frame_delta===0);
 if(!report.completed_frames_available)assert(report.completed_frames===0);
 assert(report.join_complete===Number(Boolean(report.selected_ticks>0&&![report.selected_missing_tick,report.selected_missing_deadline,report.selected_missing_wait,report.selected_missing_application,report.selected_missing_nested,report.selected_duplicate_records,report.selected_invalid_records,report.metadata_mismatches,report.sequence_range_errors].some(Boolean))));
 return report;
}

export async function readTickTraceDiagnostics(file,window,observed={}) {
 if(!validWindow(window))return validateTickTraceDiagnostics(blank(null,7,observed));
 if(!file)return validateTickTraceDiagnostics(blank(window,1,observed));
 let handle,data,status=0;
 try {
  handle=await open(file,"r");const size=(await handle.stat()).size;
  if(size>TICK_TRACE_LIMITS.bytes)status=2;
  else {
   const buffer=Buffer.alloc(TICK_TRACE_LIMITS.bytes+1);let used=0;
   while(used<buffer.length){const {bytesRead}=await handle.read(buffer,used,buffer.length-used,null);if(!bytesRead)break;used+=bytesRead;}
   if(used>TICK_TRACE_LIMITS.bytes)status=2;else data=buffer.subarray(0,used).toString("utf8");
  }
 }catch(error){status=error.code==="ENOENT"?1:6;}finally{await handle?.close();}
 if(status){const report=blank(window,status,observed);if(status===2)report.log_available=1;return validateTickTraceDiagnostics(report);}
 // Parser/validator failures are programming errors, never disguised as filesystem failure.
 return parseTickTraceDiagnostics(data,window,observed);
}
export async function writeTickTraceDiagnostics(destination,file,window,observed={}) {
 const report=await readTickTraceDiagnostics(file,window,observed);await mkdir(path.dirname(destination),{recursive:true});
 const temporary=`${destination}.${crypto.randomUUID()}.partial`;
 try{await writeFile(temporary,`${JSON.stringify(report,null,2)}\n`,{flag:"wx",mode:0o600});await rename(temporary,destination);}finally{await unlink(temporary).catch(()=>{});}
 return report;
}
