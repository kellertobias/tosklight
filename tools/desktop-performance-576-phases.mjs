// Inclusive initiating-thread wall timers, including scheduling/preemption. Nested metrics
// overlap and must not be summed or presented as CPU/worker-wait attribution.
// Window membership uses the tracing emission timestamp at ScopeDrop/frame end. A sampled
// frame can begin before the window; these are emitted-within-window duration observations.
import assert from "node:assert/strict";
import { open, mkdir, writeFile, rename, unlink } from "node:fs/promises";
import path from "node:path";
import crypto from "node:crypto";
import { diagnosticWindowBounds } from "./desktop-performance-576-clock.mjs";

// Stable public numeric code -> exact Rust tracing duration field (microseconds).
export const PHASE_FIELDS = Object.freeze([
  "ordered_total_micros", "capture_settle_micros", "projection_inclusive_micros",
  "speed_lock_acquire_micros", "dynamics_lock_acquire_micros", "scratch_lock_acquire_micros",
  "family_lock_acquire_micros", "family_preparation_inclusive_micros", "family_retire_micros",
  "family_finalize_micros", "reconciliation_micros", "family_initial_static_micros",
  "family_missing_fixed_bases_micros", "family_repeat_static_micros", "family_bind_and_control_micros",
  "family_sampling_before_callback_micros", "family_scalar_static_micros", "family_final_geometry_micros",
  "family_complete_deferred_micros", "family_prepare_families_micros", "family_cohort_setup_micros",
  "family_position_compose_micros", "family_owner_compose_micros", "family_ordered_finish_micros",
  "family_native_projection_micros",
]);
const MESSAGE = "bounded live output nested phase sample";
const META = ["generation", "show_revision", "sampled_at_millis", "accepted", "family_diagnostics_schema", "family_stage_calls", "family_counts"];
const ALLOWED = new Set([...PHASE_FIELDS,...META]);
export const PHASE_LIMITS = Object.freeze({ bytes:32*1024*1024, lines:100_000, lineBytes:64*1024, samples:1024, micros:400_000_000 });
export const PHASE_FULL_FREQUENCY_SAMPLE_LIMIT = 16_384;
function fullFrequencyOption(options) {
  assert(options && typeof options==="object" && !Array.isArray(options));
  assert(Object.keys(options).every(key=>key==="fullFrequency"));
  assert(!Object.hasOwn(options,"fullFrequency") || typeof options.fullFrequency==="boolean");
  return options.fullFrequency===true;
}
// read_status: 0 read, 1 missing, 2 byte cap, 3 line cap, 4 line-byte cap,
// 5 sample cap, 6 read failure, 7 no measurement window, 8 incomplete trailing line.
const TOP = ["schema_version","available","log_available","capture_complete","parse_complete","read_status","window_seconds","duration_field_count","lines_observed","phase_lines_observed","samples_before_window","samples_in_window","samples_after_window","invalid_samples","invalid_before_window","invalid_in_window","invalid_after_window","unlocalizable_samples","valid_samples_in_window"];
const METRIC = ["id","count","min_micros","mean_micros","p50_micros","p95_micros","max_micros"];
const unsigned=value => Number.isSafeInteger(value) && value >= 0;
const zeroMetric=id => ({id,count:0,min_micros:0,mean_micros:0,p50_micros:0,p95_micros:0,max_micros:0});
const epochMillis=value => typeof value === "number" && Number.isFinite(value) && value >= 0 && value <= Number.MAX_SAFE_INTEGER;
const validWindow=window => epochMillis(window?.startedAt) && epochMillis(window?.endedAt) && diagnosticWindowBounds(window)!==null;
function blank(window,status=0,fullFrequency=false) {
  return {...Object.fromEntries(TOP.map(name=>[name,0])),schema_version:fullFrequency?2:1,...(fullFrequency?{sample_limit:PHASE_FULL_FREQUENCY_SAMPLE_LIMIT}:{}),read_status:status,window_seconds:validWindow(window)?diagnosticWindowBounds(window).seconds:0,duration_field_count:PHASE_FIELDS.length,metrics:PHASE_FIELDS.map((_,id)=>zeroMetric(id))};
}
function timestamp(line) {
  const match=/^\s*(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2})(?:\.(\d{1,9}))?Z(?:\s|$)/u.exec(line);
  if(!match)return null;
  const seconds=Date.parse(`${match[1]}Z`);
  if(!unsigned(seconds) || new Date(seconds).toISOString().slice(0,19)!==match[1])return null;
  // Keep log fractions exact for boundary classification; BigInt never enters public output.
  return BigInt(seconds)*1_000_000n+BigInt((match[2]??"").padEnd(9,"0"));
}
function durationValues(line) {
  if(line.split(MESSAGE).length!==2)return null;
  const body=line.replace(MESSAGE,"");
  const headers=[...body.matchAll(/(?:^|\s)([A-Za-z_][A-Za-z0-9_]*)=/gu)];
  const fields=new Map();
  for(let index=0;index<headers.length;index++) {
    const current=headers[index],name=current[1];
    if(!ALLOWED.has(name) || fields.has(name))return null;
    const start=current.index+current[0].length,end=index+1<headers.length?headers[index+1].index:body.length;
    fields.set(name,body.slice(start,end).trim());
  }
  if(fields.size!==ALLOWED.size || fields.get("family_diagnostics_schema")!=="1" || !["true","false"].includes(fields.get("accepted")))return null;
  const values=[];
  for(const name of PHASE_FIELDS) {
    const text=fields.get(name);if(!/^\d+$/u.test(text??""))return null;
    const value=Number(text);if(!unsigned(value)||value>PHASE_LIMITS.micros)return null;
    values.push(value);
  }
  return values;
}
function summarize(values,id) {
  if(!values.length)return zeroMetric(id);
  const ordered=[...values].sort((a,b)=>a-b),at=p=>ordered[Math.max(0,Math.ceil(ordered.length*p)-1)];
  return {id,count:ordered.length,min_micros:ordered[0],mean_micros:ordered.reduce((a,b)=>a+b,0)/ordered.length,p50_micros:at(0.5),p95_micros:at(0.95),max_micros:ordered.at(-1)};
}
export function parsePhaseDiagnostics(text,window,options={}) {
  const fullFrequency=fullFrequencyOption(options),sampleLimit=fullFrequency?PHASE_FULL_FREQUENCY_SAMPLE_LIMIT:PHASE_LIMITS.samples;
  const report=blank(window,0,fullFrequency),values=PHASE_FIELDS.map(()=>[]);
  report.log_available=1;
  if(!validWindow(window)){report.read_status=7;return validatePhaseDiagnostics(report);}
  if(typeof text!=="string" || Buffer.byteLength(text)>PHASE_LIMITS.bytes){report.read_status=2;return validatePhaseDiagnostics(report);}
  const {startedAtNanos:opening,endedAtNanos:closing}=diagnosticWindowBounds(window);
  const lines=text.split("\n");if(lines.at(-1)==="")lines.pop();
  if(lines.length>PHASE_LIMITS.lines){report.read_status=3;return validatePhaseDiagnostics(report);}
  report.lines_observed=lines.length;
  report.capture_complete=Number(text.length===0 || text.endsWith("\n"));
  if(!report.capture_complete)report.read_status=8;
  for(const raw of lines) {
    if(Buffer.byteLength(raw)>PHASE_LIMITS.lineBytes){report.read_status=4;report.capture_complete=0;break;}
    const line=raw.replace(/\x1B\[[0-?]*[ -/]*[@-~]/gu,"");
    if(!line.includes(MESSAGE))continue;
    if(report.phase_lines_observed===sampleLimit){report.read_status=5;report.capture_complete=0;break;}
    report.phase_lines_observed++;
    const at=timestamp(line),durations=durationValues(line);
    if(at===null){report.unlocalizable_samples++;report.invalid_samples++;continue;}
    const scope=at<opening?"before_window":at>closing?"after_window":"in_window";
    report[`samples_${scope}`]++;
    if(!durations){report.invalid_samples++;report[`invalid_${scope}`]++;continue;}
    if(scope==="in_window") {report.valid_samples_in_window++;for(let id=0;id<values.length;id++)values[id].push(durations[id]);}
  }
  report.available=Number(report.capture_complete && report.valid_samples_in_window>0);
  report.parse_complete=Number(report.capture_complete && report.invalid_samples===0);
  if(report.available)report.metrics=values.map(summarize);
  return validatePhaseDiagnostics(report);
}
export function validatePhaseDiagnostics(report) {
  assert([1,2].includes(report.schema_version));
  const fullFrequency=report.schema_version===2,sampleLimit=fullFrequency?PHASE_FULL_FREQUENCY_SAMPLE_LIMIT:PHASE_LIMITS.samples;
  assert.deepEqual(Object.keys(report).sort(),[...TOP,"metrics",...(fullFrequency?["sample_limit"]:[])].sort(),"closed phase report");
  if(fullFrequency)assert.equal(report.sample_limit,PHASE_FULL_FREQUENCY_SAMPLE_LIMIT);
  for(const name of TOP)assert(typeof report[name]==="number" && Number.isFinite(report[name]) && report[name]>=0,"numeric phase scalar");
  assert.equal(report.duration_field_count,25);
  for(const name of ["available","log_available","capture_complete","parse_complete"])assert([0,1].includes(report[name]));
  assert(unsigned(report.read_status)&&report.read_status<=8);assert(report.window_seconds<=400);
  assert(report.capture_complete <= report.log_available);
  assert.equal(report.capture_complete,Number(report.read_status===0));
  if(report.read_status!==7)assert(report.window_seconds>0);
  else assert(report.window_seconds===0 && report.phase_lines_observed===0 && report.lines_observed===0);
  if([0,2,3,4,5,8].includes(report.read_status))assert.equal(report.log_available,1);
  if([1,6].includes(report.read_status))assert(report.log_available===0 && report.phase_lines_observed===0 && report.lines_observed===0);
  if([2,3].includes(report.read_status))assert(report.phase_lines_observed===0 && report.lines_observed===0);
  if(report.read_status===5)assert.equal(report.phase_lines_observed,sampleLimit);
  for(const name of TOP.filter(name=>!["window_seconds"].includes(name)))assert(unsigned(report[name]));
  assert(report.lines_observed<=PHASE_LIMITS.lines && report.phase_lines_observed<=sampleLimit);
  assert.equal(report.phase_lines_observed,report.samples_before_window+report.samples_in_window+report.samples_after_window+report.unlocalizable_samples);
  assert.equal(report.invalid_samples,report.invalid_before_window+report.invalid_in_window+report.invalid_after_window+report.unlocalizable_samples);
  for(const scope of ["before_window","in_window","after_window"])assert(report[`invalid_${scope}`]<=report[`samples_${scope}`]);
  assert.equal(report.valid_samples_in_window,report.samples_in_window-report.invalid_in_window);
  assert.equal(report.available,Number(Boolean(report.capture_complete && report.valid_samples_in_window>0)));
  assert.equal(report.parse_complete,Number(Boolean(report.capture_complete && report.invalid_samples===0)));
  assert(Array.isArray(report.metrics)&&report.metrics.length===25);
  for(const [id,row] of report.metrics.entries()) {
    assert.deepEqual(Object.keys(row).sort(),METRIC.slice().sort());assert.equal(row.id,id);
    for(const value of Object.values(row))assert(typeof value==="number"&&Number.isFinite(value)&&value>=0);
    assert(unsigned(row.count)&&row.count===(report.available?report.valid_samples_in_window:0));
    if(!row.count)assert(METRIC.filter(name=>name!=="id").every(name=>row[name]===0));
    else assert([row.min_micros,row.p50_micros,row.p95_micros,row.max_micros].every(unsigned) && row.min_micros<=row.p50_micros && row.p50_micros<=row.p95_micros && row.p95_micros<=row.max_micros && row.min_micros<=row.mean_micros && row.mean_micros<=row.max_micros && row.max_micros<=PHASE_LIMITS.micros);
  }
  return report;
}
export async function readPhaseDiagnostics(file,window,options={}) {
  const fullFrequency=fullFrequencyOption(options);
  if(!validWindow(window))return validatePhaseDiagnostics(blank(window,7,fullFrequency));
  let handle;
  try {
    handle=await open(file,"r");const info=await handle.stat();
    if(!info.isFile())return validatePhaseDiagnostics(blank(window,6,fullFrequency));
    if(info.size>PHASE_LIMITS.bytes)return validatePhaseDiagnostics({...blank(window,2,fullFrequency),log_available:1});
    const bytes=Buffer.alloc(info.size);let offset=0;
    while(offset<bytes.length){const {bytesRead}=await handle.read(bytes,offset,bytes.length-offset,offset);if(!bytesRead)break;offset+=bytesRead;}
    const report=parsePhaseDiagnostics(bytes.subarray(0,offset).toString("utf8"),window,options);
    if(offset<info.size){report.read_status=8;report.capture_complete=0;report.parse_complete=0;report.available=0;report.metrics=PHASE_FIELDS.map((_,id)=>zeroMetric(id));}
    return validatePhaseDiagnostics(report);
  } catch(error) {return validatePhaseDiagnostics(blank(window,error?.code==="ENOENT"?1:6,fullFrequency));}
  finally {await handle?.close().catch(()=>{});}
}
export async function writePhaseDiagnostics(destination,file,window,options={}) {
  const report=await readPhaseDiagnostics(file,window,options);
  await mkdir(path.dirname(destination),{recursive:true});
  const temporary=`${destination}.${crypto.randomUUID()}.tmp`;
  try {await writeFile(temporary,JSON.stringify(report,null,2)+"\n",{flag:"wx",mode:0o600});await rename(temporary,destination);}
  catch(error){await unlink(temporary).catch(()=>{});throw error;}
  return report;
}
