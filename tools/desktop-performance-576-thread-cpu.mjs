// Owned Linux task CPU counters. Role bins are thread families, never exclusive application stages.
// CPU numerators sum only adjacent, same-identity task observations. Partial reports retain
// observed CPU over the full census-midpoint window; missing intervals/tasks are not estimated.
// /proc values are tick-quantized; per-task reads lie within the reported census brackets.
// Neither the Linux scheduler caller nor pool cleanup is an independently named thread role.
import assert from "node:assert/strict";
import * as fs from "node:fs/promises";
import path from "node:path";
import crypto from "node:crypto";
import {bindOwnedHeadless} from "./desktop-performance-576-workers.mjs";
export const THREAD_CPU_LIMITS = Object.freeze({observations:61,tasks:256,statBytes:8192,censusMillis:2000});
export const THREAD_CPU_ROLE_IDS = Object.freeze([0,1,2,3,4]); // leader, output pool, Tokio family, persistence, other
const uint=n=>Number.isSafeInteger(n)&&n>=0;
const closed=(r,keys)=>assert.deepEqual(Object.keys(r).sort(),keys.slice().sort());
const us=n=>Math.ceil(n*1000);
function check(ok,code=2){if(!ok)throw Object.assign(new Error("thread CPU observation unavailable"),{diagnosticCode:code});}
export function threadCpuEnabled(environment,mode){return environment.LIGHT_OUTPUT_THREAD_CPU_DIAGNOSTICS==="1"&&environment.LIGHT_OUTPUT_PHASE_DIAGNOSTICS==="all"&&mode==="control-no-stage";}
export function verifiedClockTicks(result){const text=result?.stdout?.trim();return result?.status===0&&/^\d+$/u.test(text??"")&&uint(Number(text))&&Number(text)>0&&Number(text)<=1_000_000?Number(text):null;}
export function parseTaskStat(text){
 check(typeof text==="string"&&Buffer.byteLength(text)<=THREAD_CPU_LIMITS.statBytes);
 const m=/^([1-9]\d*) \(([^\r\n\0]*)\) ([A-Za-z]) (.*)\n?$/u.exec(text);check(m);
 const f=(m[3]+" "+m[4]).trim().split(/\s+/u);check(f.length>=22&&Buffer.byteLength(m[2])>0&&Buffer.byteLength(m[2])<=15);
 const number=index=>{check(/^\d+$/u.test(f[index]));const value=Number(f[index]);check(uint(value));return value;};
 const tid=Number(m[1]);check(uint(tid)&&tid>0);
 return {tid,comm:m[2],startTicks:number(19),userTicks:number(11),systemTicks:number(12)};
}
export function taskRole(task,pid){
 if(task.tid===pid)return 0;
 if(/^light-output-(?:0|[1-9]\d?)$/u.test(task.comm))return 1;
 if(task.comm==="tokio-runtime-w")return 2;
 if(task.comm==="light-programme")return 3; // Linux truncation of light-programmer-persistence.
 return 4;
}
function identity(stat){return [stat.dev,stat.ino,stat.size,stat.mtimeNs,stat.ctimeNs].map(String).join(":");}
async function capture(binding,io,now,utc){
 const start=now(),utcStart=utc();check(Number.isFinite(start)&&start>=0&&uint(utcStart),5);
 let previousTime=start;
 const time=()=>{const current=now();check(Number.isFinite(current)&&current>=previousTime,5);previousTime=current;check(current-start<=THREAD_CPU_LIMITS.censusMillis,9);return current;};
 const bounded=()=>{time();};
 const process=async()=>{bounded();const p=parseTaskStat(await io.readFile(`/proc/${binding.pid}/stat`,"utf8"));const root=parseTaskStat(await io.readFile(`/proc/${binding.rootPid}/stat`,"utf8"));check(p.tid===binding.pid&&p.startTicks===binding.startTicks&&p.comm==="light-headless"&&root.tid===binding.rootPid&&root.startTicks===binding.rootStartTicks,3);return p;};
 const executable=async()=>{bounded();const file=`/proc/${binding.pid}/exe`,link=await io.readlink(file);check(typeof link==="string"&&link.length>0&&link.length<=4096&&!link.endsWith(" (deleted)"),3);const stat=await io.stat(file,{bigint:true});check(stat.isFile()&&[stat.dev,stat.ino,stat.size,stat.mtimeNs,stat.ctimeNs].every(n=>typeof n==="bigint"&&n>=0n)&&stat.size>0n,3);return {link,identity:identity(stat)};};
 const tasks=async()=>{bounded();const ids=await io.readdir(`/proc/${binding.pid}/task`);check(Array.isArray(ids)&&ids.length>0&&ids.length<=THREAD_CPU_LIMITS.tasks,4);check(ids.every(id=>/^[1-9]\d*$/u.test(id)&&uint(Number(id)))&&new Set(ids).size===ids.length,2);return ids.sort((a,b)=>Number(a)-Number(b));};
 const before=await process(),exe=await executable(),ids=await tasks(),rows=[];
 for(const id of ids){bounded();const readStart=time();const task=parseTaskStat(await io.readFile(`/proc/${binding.pid}/task/${id}/stat`,"utf8"));check(task.tid===Number(id),3);const readEnd=time();check(readEnd>=readStart,5);rows.push({...task,role:taskRole(task,binding.pid),readStart,readEnd});}
 const afterIds=await tasks(),exeAfter=await executable(),after=await process();
 check(JSON.stringify(ids)===JSON.stringify(afterIds),6);check(exe.link===exeAfter.link&&exe.identity===exeAfter.identity,3);
 check(after.userTicks>=before.userTicks&&after.systemTicks>=before.systemTicks,7);
 const end=time(),utcEnd=utc();bounded();check(end>=start&&uint(utcEnd)&&utcEnd>=utcStart,5);
 return {start,end,utcStart,utcEnd,exe,rows,processBefore:before,processAfter:after};
}
const ROLE_KEYS=["id","threads_first","threads_last","matched_intervals","user_ticks","system_ticks","cpu_micros","cpu_percent","observed_share_percent"];
const TOP=["schema_version","available","capture_complete","cpu_available","hz_verified","ticks_per_second","tick_quantization_micros","observations","valid_observations","valid_intervals","failure_code","partial_coverage","identity_verified","exe_identity_verified","output_caller_role_available","cleanup_role_available","pool_includes_cleanup_sort_search","stable_tids_only","sample_midpoint_approximation","exact_server_window","partial_rates_use_full_window","covered_window_micros","window_micros","minimum_window_micros","maximum_window_micros","server_window_micros","census_total_micros","census_max_micros","max_after_server_micros","new_threads","exited_threads","reused_threads","renamed_threads","unreadable_censuses","counter_failures","unresolved_threads","roles"];
function blank(hz){return {schema_version:1,available:0,capture_complete:0,cpu_available:0,hz_verified:Number(hz!==null),ticks_per_second:hz??0,tick_quantization_micros:hz===null?0:Math.ceil(1e6/hz),observations:0,valid_observations:0,valid_intervals:0,failure_code:0,partial_coverage:0,identity_verified:0,exe_identity_verified:0,output_caller_role_available:0,cleanup_role_available:0,pool_includes_cleanup_sort_search:1,stable_tids_only:1,sample_midpoint_approximation:1,exact_server_window:0,partial_rates_use_full_window:1,covered_window_micros:0,window_micros:0,minimum_window_micros:0,maximum_window_micros:0,server_window_micros:0,census_total_micros:0,census_max_micros:0,max_after_server_micros:0,new_threads:0,exited_threads:0,reused_threads:0,renamed_threads:0,unreadable_censuses:0,counter_failures:0,unresolved_threads:0,roles:THREAD_CPU_ROLE_IDS.map(id=>({id,threads_first:0,threads_last:0,matched_intervals:0,user_ticks:0,system_ticks:0,cpu_micros:0,cpu_percent:0,observed_share_percent:0}))};}
export class ThreadCpuDiagnostics {
 constructor(hz,{io=fs,now=()=>performance.now(),utc=()=>Date.now()}={}){assert(hz===null||uint(hz)&&hz>0&&hz<=1_000_000);this.hz=hz;this.io=io;this.now=now;this.utc=utc;this.binding=null;this.records=[];this.observations=0;this.failure=0;}
 async observe(snapshot,rootPid,sample){
  if(++this.observations>61){this.failure ||=4;return;}
  try{
   const binding=bindOwnedHeadless(snapshot,rootPid);check(!this.binding||JSON.stringify(binding)===JSON.stringify(this.binding),3);
   const server=sample?.diagnostics?.output_sample;check(uint(server?.sampled_at_epoch_micros)&&uint(server?.monotonic_micros),5);
   const record=await capture(binding,this.io,this.now,this.utc);check(record.utcStart*1000+1000>=server.sampled_at_epoch_micros,5);
   const anchor=this.records.find(x=>x.valid);if(anchor)check(Math.abs((record.utcStart-anchor.utcStart)-(record.start-anchor.start))<=2&&Math.abs((record.utcEnd-anchor.utcEnd)-(record.end-anchor.end))<=2,5);
   const previous=this.records.at(-1);if(previous?.valid){check(record.start>previous.end&&server.monotonic_micros>previous.server.monotonic_micros,5);check(JSON.stringify(record.exe)===JSON.stringify(previous.exe),3);}
   this.binding=binding;this.records.push({...record,server,valid:true});
  }catch(error){const code=error.diagnosticCode??2;this.failure ||=code;this.records.push({valid:false,code});}
 }
 privateState(){return {binding:this.binding,records:this.records};}
 summary(){
  const r=blank(this.hz);r.observations=Math.min(this.observations,62);r.failure_code=this.failure;r.valid_observations=this.records.filter(x=>x.valid).length;r.unreadable_censuses=this.records.filter(x=>!x.valid).length;
  const valid=this.records.filter(x=>x.valid);r.identity_verified=r.exe_identity_verified=Number(valid.length>0&&!this.records.some(x=>x.code===3));
  for(const sample of valid){const duration=us(sample.end-sample.start);r.census_total_micros+=duration;r.census_max_micros=Math.max(r.census_max_micros,duration);r.max_after_server_micros=Math.max(r.max_after_server_micros,Math.max(0,sample.utcEnd*1000-sample.server.sampled_at_epoch_micros));}
  if(valid.length>1){const first=valid[0],last=valid.at(-1);r.window_micros=us((last.start+last.end-first.start-first.end)/2);r.minimum_window_micros=Math.max(0,Math.floor((last.start-first.end)*1000));r.maximum_window_micros=us(last.end-first.start);r.server_window_micros=last.server.monotonic_micros-first.server.monotonic_micros;}
  const first=this.records[0],last=this.records.at(-1);
  for(const [sample,key] of [[first,"threads_first"],[last,"threads_last"]])if(sample?.valid)for(const row of sample.rows)r.roles[row.role][key]++;
  const invalidCounterTasks=new Set();
  for(let i=1;i<this.records.length;i++){
   const a=this.records[i-1],b=this.records[i];if(!a.valid||!b.valid)continue;
   const previous=new Map(a.rows.map(row=>[row.tid,row])),current=new Map(b.rows.map(row=>[row.tid,row]));r.valid_intervals++;r.covered_window_micros+=us((b.start+b.end-a.start-a.end)/2);
   for(const row of b.rows){const before=previous.get(row.tid);if(!before){r.new_threads++;continue;}if(row.startTicks!==before.startTicks){r.reused_threads++;continue;}if(row.comm!==before.comm){r.renamed_threads++;continue;}const identity=row.tid+":"+row.startTicks;if(invalidCounterTasks.has(identity))continue;if(row.userTicks<before.userTicks||row.systemTicks<before.systemTicks){r.counter_failures++;invalidCounterTasks.add(identity);continue;}const out=r.roles[row.role];out.user_ticks+=row.userTicks-before.userTicks;out.system_ticks+=row.systemTicks-before.systemTicks;out.matched_intervals++;}
   for(const row of a.rows)if(!current.has(row.tid))r.exited_threads++;
  }
  r.unresolved_threads=r.roles[4].threads_last;
  r.partial_coverage=Number(Boolean(r.new_threads+r.exited_threads+r.reused_threads+r.renamed_threads+r.unreadable_censuses+r.counter_failures));
  if(r.counter_failures)r.failure_code ||=7;
  r.capture_complete=Number(r.observations===61&&r.valid_observations===61&&!r.failure_code);
  r.cpu_available=Number(r.hz_verified&&r.valid_intervals>0&&r.window_micros>0&&!r.counter_failures);
  const total=r.roles.reduce((n,row)=>n+row.user_ticks+row.system_ticks,0);
  for(const row of r.roles){const ticks=row.user_ticks+row.system_ticks;if(r.cpu_available){row.cpu_micros=ticks/r.ticks_per_second*1e6;row.cpu_percent=row.cpu_micros/r.window_micros*100;row.observed_share_percent=total?ticks/total*100:0;}}
  r.available=Number(r.capture_complete&&r.cpu_available&&!r.partial_coverage&&r.identity_verified&&r.exe_identity_verified);
  return validateThreadCpuDiagnostics(r);
 }
}
export function validateThreadCpuDiagnostics(r){
 closed(r,TOP);for(const [key,value]of Object.entries(r))if(key!=="roles")assert(typeof value==="number"&&Number.isFinite(value)&&value>=0);
 for(const key of TOP.filter(k=>!['roles','tick_quantization_micros'].includes(k)))assert(uint(r[key]));
 for(const key of ['available','capture_complete','cpu_available','hz_verified','partial_coverage','identity_verified','exe_identity_verified'])assert(r[key]<=1);
 assert(r.schema_version===1&&r.observations<=62&&r.valid_observations<=61&&r.valid_observations+r.unreadable_censuses===Math.min(r.observations,61)&&r.valid_intervals<=Math.max(0,r.valid_observations-1)&&r.failure_code<=9);
 assert(r.output_caller_role_available===0&&r.cleanup_role_available===0&&r.pool_includes_cleanup_sort_search===1&&r.stable_tids_only===1&&r.sample_midpoint_approximation===1&&r.exact_server_window===0&&r.partial_rates_use_full_window===1);
 assert(r.hz_verified===Number(r.ticks_per_second>0)&&r.ticks_per_second<=1_000_000&&r.tick_quantization_micros===(r.hz_verified?Math.ceil(1e6/r.ticks_per_second):0));
 assert(r.minimum_window_micros<=r.window_micros&&r.window_micros<=r.maximum_window_micros&&r.maximum_window_micros<=401e6&&r.census_max_micros<=2e6&&r.census_total_micros<=122e6);
 assert(r.capture_complete===Number(r.observations===61&&r.valid_observations===61&&!r.failure_code));assert(r.cpu_available===Number(r.hz_verified&&r.valid_intervals>0&&r.window_micros>0&&!r.counter_failures));
 assert(r.partial_coverage===Number(Boolean(r.new_threads+r.exited_threads+r.reused_threads+r.renamed_threads+r.unreadable_censuses+r.counter_failures)));
 assert(r.available===Number(r.capture_complete&&r.cpu_available&&!r.partial_coverage&&r.identity_verified&&r.exe_identity_verified));
 assert(Array.isArray(r.roles)&&r.roles.length===5);const total=r.roles.reduce((n,row)=>n+row.user_ticks+row.system_ticks,0);assert(uint(total));
 r.roles.forEach((row,id)=>{closed(row,ROLE_KEYS);assert(row.id===id);for(const key of ROLE_KEYS){assert(typeof row[key]==="number"&&Number.isFinite(row[key])&&row[key]>=0);if(!['cpu_micros','cpu_percent','observed_share_percent'].includes(key))assert(uint(row[key]));}assert(row.threads_first<=256&&row.threads_last<=256&&row.matched_intervals<=256*60);const ticks=row.user_ticks+row.system_ticks;assert(row.cpu_micros===(r.cpu_available?ticks/r.ticks_per_second*1e6:0));assert(row.cpu_percent===(r.cpu_available?row.cpu_micros/r.window_micros*100:0));assert(row.observed_share_percent===(r.cpu_available&&total?ticks/total*100:0));});
 assert(r.roles.reduce((n,row)=>n+row.threads_first,0)<=256&&r.roles.reduce((n,row)=>n+row.threads_last,0)<=256&&r.unresolved_threads===r.roles[4].threads_last);
 assert(r.covered_window_micros<=r.window_micros+61&&r.max_after_server_micros<=400e6);
 if(r.capture_complete){assert(r.valid_intervals===60);assert(r.roles[0].threads_first===1&&r.roles[0].threads_last===1);}
 if(r.available)for(const row of r.roles)assert(row.threads_first===row.threads_last&&row.matched_intervals===row.threads_first*60);
 if(r.unreadable_censuses)assert(r.failure_code>0);if(r.observations===62)assert(r.failure_code>0);if(r.counter_failures)assert(r.failure_code>0);
 return r;
}
export async function writeThreadCpuDiagnostics(destination,collector){const report=collector.summary();await fs.mkdir(path.dirname(destination),{recursive:true});const temporary=destination+"."+crypto.randomUUID()+".partial";try{await fs.writeFile(temporary,JSON.stringify(report,null,2)+"\n",{flag:"wx",mode:0o600});await fs.rename(temporary,destination);}finally{await fs.unlink(temporary).catch(()=>{});}return report;}
