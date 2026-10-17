// Actual owned-process thread witnesses. Raw identities/names remain private; this is not a rate gate.
import assert from "node:assert/strict";
import {readFile,readdir,mkdir,writeFile,rename,unlink} from "node:fs/promises";
import path from "node:path";
import crypto from "node:crypto";
export const WORKER_LIMITS=Object.freeze({tasks:4096,samples:2,commBytes:16});
// failure_code: 0 none, 1 snapshot/binding, 2 proc read, 3 identity changed,
// 4 task cap, 5 thread set changed, 6 malformed comm, 7 unexpected geometry, 8 order/count.
const TOP=["schema_version","expected_workers","available","capture_complete","observations","valid_observations","failure_code","samples"];
const ROW=["id","available","matches_expected","failure_code","task_count","output_thread_count","pool_count","index_counts"];
const uint=n=>Number.isSafeInteger(n)&&n>=0,positive=n=>uint(n)&&n>0;
const closed=(r,keys)=>assert.deepEqual(Object.keys(r).sort(),keys.slice().sort());
class ReceiptFailure extends Error {constructor(code,message){super(message);this.code=code;}}
const fail=(code,message)=>{throw new ReceiptFailure(code,message);};
function geometry(counts,expected){return expected===1?counts.every(n=>n===0):counts[0]>0&&counts.every((n,i)=>i<expected?n===counts[0]:n===0);}
const empty=id=>({id,available:0,matches_expected:0,failure_code:0,task_count:0,output_thread_count:0,pool_count:0,index_counts:[0,0,0,0]});
export function bindOwnedHeadless(snapshot,rootPid){
 if(!positive(rootPid)||!Array.isArray(snapshot)||snapshot.length>WORKER_LIMITS.tasks)fail(1,"invalid owned process snapshot");
 const byPid=new Map();
 for(const p of snapshot){if(!p||!positive(p.pid)||!uint(p.startTicks)||!uint(p.parentPid)||typeof p.command!=="string"||byPid.has(p.pid))fail(1,"malformed process identity");byPid.set(p.pid,p);}
 const root=byPid.get(rootPid);if(!root)fail(1,"owned root missing");
 const candidates=snapshot.filter(p=>p.command==="light-headless"&&p.pid!==rootPid).filter(p=>{
  const seen=new Set();let current=p;
  while(current&&current.pid!==rootPid){if(seen.has(current.pid))fail(1,"cyclic process ancestry");seen.add(current.pid);current=byPid.get(current.parentPid);}
  return current?.pid===rootPid;
 });
 if(candidates.length!==1)fail(1,"missing or ambiguous owned headless process");
 return {pid:candidates[0].pid,startTicks:candidates[0].startTicks,rootPid,rootStartTicks:root.startTicks};
}
function statIdentity(text){
 if(typeof text!=="string"||Buffer.byteLength(text)>4096)fail(3,"malformed process stat");
 const close=text.lastIndexOf(")"),open=text.indexOf(" (");
 const pid=text.slice(0,open),fields=text.slice(close+2).trim().split(/\s+/u);
 if(open<1||close<=open||!/^\d+$/u.test(pid)||!/^\d+$/u.test(fields[19]??""))fail(3,"malformed process identity");
 const result={pid:Number(pid),startTicks:Number(fields[19]),command:text.slice(open+2,close)};
 if(!positive(result.pid)||!uint(result.startTicks))fail(3,"unsafe process identity");return result;
}
export function parseOutputWorkerNames(names,expected){
 assert([1,2,3,4].includes(expected));
 if(!Array.isArray(names)||names.length===0||names.length>WORKER_LIMITS.tasks)fail(4,"invalid task count");
 const counts=[0,0,0,0];
 for(const comm of names){
  if(typeof comm!=="string"||!comm.endsWith("\n")||Buffer.byteLength(comm)>WORKER_LIMITS.commBytes||comm.slice(0,-1).length===0||/[\n\r\0]/u.test(comm.slice(0,-1)))fail(6,"malformed task comm");
  const name=comm.slice(0,-1);if(!name.startsWith("light-output"))continue;
  const match=/^light-output-([0-3])$/u.exec(name);if(!match)fail(6,"malformed output worker index");counts[Number(match[1])]++;
 }
 const matches=Number(geometry(counts,expected));
 return {task_count:names.length,output_thread_count:counts.reduce((a,b)=>a+b,0),pool_count:matches&&expected>1?counts[0]:0,index_counts:counts,matches_expected:matches};
}
export async function captureOwnedWorkerThreads(binding,io={readFile,readdir}){
 const check=async()=>{
  const p=statIdentity(await io.readFile(`/proc/${binding.pid}/stat`,"utf8"));
  const root=statIdentity(await io.readFile(`/proc/${binding.rootPid}/stat`,"utf8"));
  if(p.pid!==binding.pid||p.startTicks!==binding.startTicks||p.command!=="light-headless"||root.pid!==binding.rootPid||root.startTicks!==binding.rootStartTicks)fail(3,"owned process identity changed");
 };
 const tasks=async()=>{
  const ids=await io.readdir(`/proc/${binding.pid}/task`);
  if(!Array.isArray(ids)||ids.length===0||ids.length>WORKER_LIMITS.tasks)fail(4,"task count cap");
  if(ids.some(id=>typeof id!=="string"||!/^[1-9]\d*$/u.test(id)||!positive(Number(id)))||new Set(ids).size!==ids.length)fail(5,"malformed task inventory");
  return ids.slice().sort((a,b)=>Number(a)-Number(b));
 };
 await check();const ids=await tasks(),names=[];
 // Sequential bounded reads avoid launching thousands of simultaneous file operations.
 for(const id of ids)names.push(await io.readFile(`/proc/${binding.pid}/task/${id}/comm`,"utf8"));
 const after=await tasks();await check();if(ids.length!==after.length||ids.some((id,i)=>id!==after[i]))fail(5,"task inventory changed");
 return {ids,names};
}
export class WorkerCountReceipt {
 constructor(expected,{io={readFile,readdir},now=()=>performance.now()}={}){assert([1,2,3,4].includes(expected));this.expected=expected;this.io=io;this.now=now;this.observations=0;this.failure=0;this.rows=[empty(0),empty(1)];this.binding=null;this.previousEnd=null;this.private=[];}
 async observe(snapshot,rootPid){
  if(this.observations>=2){this.observations=3;this.failure ||=8;return this.summary();}
  const id=this.observations++,record={id,startedAt:this.now()};this.private.push(record);
  try{
   if(!Number.isFinite(record.startedAt)||record.startedAt<0||this.previousEnd!==null&&record.startedAt<this.previousEnd)fail(8,"reordered worker observations");
   const binding=bindOwnedHeadless(snapshot,rootPid);record.binding=binding;
   if(this.binding&&Object.keys(binding).some(k=>binding[k]!==this.binding[k]))fail(3,"headless binding changed between observations");
   this.binding??=binding;
   const captured=await captureOwnedWorkerThreads(binding,this.io);Object.assign(record,captured);
   const data=parseOutputWorkerNames(captured.names,this.expected);this.rows[id]={...empty(id),...data,available:1,failure_code:data.matches_expected?0:7};
  }catch(error){const code=error instanceof ReceiptFailure?error.code:2;this.rows[id].failure_code=code;record.error=String(error);}
  record.endedAt=this.now();
  if(!Number.isFinite(record.endedAt)||record.endedAt<record.startedAt){this.rows[id]={...empty(id),failure_code:8};}
  this.previousEnd=record.endedAt;this.failure ||=this.rows[id].failure_code;return this.summary();
 }
 summary(){const valid=this.rows.reduce((n,r)=>n+r.matches_expected,0),capture=Number(this.observations===2&&this.rows.every(r=>r.available));return validateWorkerDiagnostics({schema_version:1,expected_workers:this.expected,available:Number(capture&&valid===2&&!this.failure),capture_complete:capture,observations:this.observations,valid_observations:valid,failure_code:this.failure,samples:structuredClone(this.rows)});}
 privateState(){return structuredClone(this.private);}
}
export function validateWorkerDiagnostics(r){
 closed(r,TOP);assert(r.schema_version===1&&[1,2,3,4].includes(r.expected_workers));
 for(const k of TOP.filter(k=>k!=="samples"))assert(uint(r[k]));assert(r.observations<=3&&r.failure_code<=8&&r.samples.length===2);
 for(const [id,row]of r.samples.entries()){
  closed(row,ROW);assert(row.id===id);for(const k of ROW.filter(k=>k!=="index_counts"))assert(uint(row[k]));assert(row.failure_code<=8&&row.available<=1&&row.matches_expected<=1&&row.task_count<=WORKER_LIMITS.tasks);
  assert(Array.isArray(row.index_counts)&&row.index_counts.length===4&&row.index_counts.every(uint));assert(row.output_thread_count===row.index_counts.reduce((a,b)=>a+b,0)&&row.output_thread_count<=row.task_count);
  if(row.available){assert(row.task_count>0);const match=Number(geometry(row.index_counts,r.expected_workers));assert(row.matches_expected===match&&row.failure_code===(match?0:7));assert(row.pool_count===(match&&r.expected_workers>1?row.index_counts[0]:0));}
  else assert(!row.matches_expected&&!row.task_count&&!row.output_thread_count&&!row.pool_count&&row.index_counts.every(n=>n===0));
  if(id>=r.observations)assert.deepEqual(row,empty(id));
 }
 assert(r.valid_observations===r.samples.reduce((n,row)=>n+row.matches_expected,0));assert(r.capture_complete===Number(r.observations===2&&r.samples.every(row=>row.available)));
 const first=r.samples.find(row=>row.failure_code)?.failure_code??0;assert(r.failure_code===(first||Number(r.observations>2)*8));
 assert(r.available===Number(Boolean(r.capture_complete&&r.valid_observations===2&&!r.failure_code)));return r;
}
export async function writeWorkerDiagnostics(destination,report){
 validateWorkerDiagnostics(report);await mkdir(path.dirname(destination),{recursive:true});const temporary=destination+"."+crypto.randomUUID()+".partial";
 try{await writeFile(temporary,JSON.stringify(report,null,2)+"\n",{flag:"wx",mode:0o600});await rename(temporary,destination);}finally{await unlink(temporary).catch(()=>{});}return report;
}
