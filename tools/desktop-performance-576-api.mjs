// These are 61 sampled LAST values, not request timing distributions, total CPU, or
// coherent per-request records. Independent atomic stores may belong to different requests.
import assert from 'node:assert/strict';
import {mkdir,writeFile,rename,unlink} from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import {ServerCounterClock,emptyClockSummary,validateClockSummary} from './desktop-performance-576-clock.mjs';

export const API_LIMITS=Object.freeze({observations:61,windowSeconds:400});
export const API_COUNTERS=Object.freeze(['projections','snapshot_requests','stream_serializations','stream_sends']);
export const API_LAST_METRICS=Object.freeze(['projection_micros','payload_bytes','snapshot_projection_micros','snapshot_serialization_micros','snapshot_payload_bytes','stream_serialization_micros','stream_payload_bytes','normal_subscribers','preload_subscribers']);
export const API_OUTPUT_COUNTERS=Object.freeze(['completed_frames','deadline_misses','send_errors','packets_sent']);
const TOP=['schema_version','available','read_status','observations','clock_validation_passed','window_seconds','window_available','output_counters_available',...API_OUTPUT_COUNTERS,'sampled_last_values','per_request_timing_available','per_request_coherence_available','process_cpu_available','clock','counters','last_values'];
const STAT=['id','count','min','mean','p50','p95','max'];
const uint=n=>Number.isSafeInteger(n)&&n>=0;
const closed=(value,keys)=>assert.deepEqual(Object.keys(value).sort(),keys.slice().sort());
const zero=id=>({id,count:0,min:0,mean:0,p50:0,p95:0,max:0});
function blank(){return {schema_version:1,available:0,read_status:1,observations:0,clock_validation_passed:0,window_seconds:0,window_available:0,output_counters_available:0,...Object.fromEntries(API_OUTPUT_COUNTERS.map(k=>[k,0])),sampled_last_values:1,per_request_timing_available:0,per_request_coherence_available:0,process_cpu_available:0,clock:emptyClockSummary(),counters:API_COUNTERS.map((_,id)=>({id,available:0,delta:0})),last_values:API_LAST_METRICS.map((_,id)=>zero(id))};}
function summarize(values,id){const v=values.slice().sort((a,b)=>a-b),at=p=>v[Math.ceil(v.length*p)-1];let mean=0;for(let i=0;i<v.length;i++)mean+=(v[i]-mean)/(i+1);return {id,count:v.length,min:v[0],mean:Math.max(v[0],Math.min(v.at(-1),mean)),p50:at(.5),p95:at(.95),max:v.at(-1)};}
export function summarizeApiDiagnostics(samples){
 const r=blank();
 if(!Array.isArray(samples))return validateApiDiagnostics(r);
 r.observations=Math.min(samples.length,API_LIMITS.observations+1);
 if(samples.length!==API_LIMITS.observations){r.read_status=2;return validateApiDiagnostics(r);}
 const clock=new ServerCounterClock();
 try{for(const sample of samples)clock.observe(sample);r.clock=clock.summary();r.clock_validation_passed=r.clock.clock_validation_passed;r.window_seconds=clock.elapsedSeconds();assert(r.window_seconds>0&&r.window_seconds<=API_LIMITS.windowSeconds);r.window_available=1;}catch{r.clock=clock.summary();r.clock_validation_passed=r.clock.clock_validation_passed;r.window_seconds=0;r.window_available=0;r.read_status=3;return validateApiDiagnostics(r);}
 const counters=API_COUNTERS.map(()=>[]),values=API_LAST_METRICS.map(()=>[]);
 for(const sample of samples){const v=sample?.diagnostics?.visualization;
  if(!v||typeof v!=='object'||Array.isArray(v)){r.read_status=4;return validateApiDiagnostics(r);}
  for(const [i,key]of API_COUNTERS.entries()){const n=v[key];if(!uint(n)||counters[i].length&&n<counters[i].at(-1)){r.read_status=4;return validateApiDiagnostics(r);}counters[i].push(n);}
  for(const [i,key]of API_LAST_METRICS.entries()){const n=v[key];if(!uint(n)){r.read_status=5;return validateApiDiagnostics(r);}values[i].push(n);}
 }
 r.counters=counters.map((v,id)=>({id,available:1,delta:v.at(-1)-v[0]}));r.last_values=values.map(summarize);r.read_status=0;r.available=1;r.output_counters_available=1;Object.assign(r,{completed_frames:clock.delta('frames_sent'),deadline_misses:clock.delta('deadline_misses'),send_errors:clock.delta('send_errors'),packets_sent:clock.delta('packets_sent')});return validateApiDiagnostics(r);
}
export function validateApiDiagnostics(r){
 closed(r,TOP);assert(r.schema_version===1&&[0,1].includes(r.available)&&uint(r.read_status)&&r.read_status<=5&&uint(r.observations)&&r.observations<=API_LIMITS.observations+1);assert([0,1].includes(r.clock_validation_passed));assert(typeof r.window_seconds==='number'&&Number.isFinite(r.window_seconds)&&r.window_seconds>=0&&r.window_seconds<=API_LIMITS.windowSeconds);
 assert(r.window_available===Number(r.window_seconds>0)&&r.output_counters_available===r.available);for(const key of API_OUTPUT_COUNTERS){assert(uint(r[key]));if(!r.output_counters_available)assert(r[key]===0);}
 assert(r.sampled_last_values===1&&r.per_request_timing_available===0&&r.per_request_coherence_available===0&&r.process_cpu_available===0);validateClockSummary(r.clock);assert(r.clock_validation_passed===r.clock.clock_validation_passed);assert(r.available===Number(r.read_status===0));
 if(r.read_status===1)assert(r.observations===0);if(r.read_status===2)assert(r.observations!==61);if(r.read_status>=3||r.available)assert(r.observations===61);
 if(r.available||r.read_status>=4)assert(r.clock_validation_passed===1&&r.window_seconds>0);else assert(r.window_seconds===0);
 if(r.read_status>0&&r.read_status<=2)assert.deepEqual(r.clock,emptyClockSummary());
 assert(Array.isArray(r.counters)&&r.counters.length===4);for(const [id,row]of r.counters.entries()){closed(row,['id','available','delta']);assert(row.id===id&&row.available===r.available&&uint(row.delta));if(!row.available)assert(row.delta===0);}
 assert(Array.isArray(r.last_values)&&r.last_values.length===9);for(const [id,row]of r.last_values.entries()){closed(row,STAT);assert(row.id===id&&row.count===(r.available?61:0));for(const key of ['min','p50','p95','max'])assert(uint(row[key]));assert(typeof row.mean==='number'&&Number.isFinite(row.mean)&&row.mean>=0&&row.mean<=Number.MAX_SAFE_INTEGER);assert(row.min<=row.p50&&row.p50<=row.p95&&row.p95<=row.max&&row.min<=row.mean&&row.mean<=row.max);if(!row.count)assert(STAT.slice(2).every(key=>row[key]===0));}
 return r;
}
export async function writeApiDiagnostics(destination,samples){const report=summarizeApiDiagnostics(samples);await mkdir(path.dirname(destination),{recursive:true});const temporary=destination+'.'+crypto.randomUUID()+'.partial';try{await writeFile(temporary,JSON.stringify(report,null,2)+'\n',{flag:'wx',mode:0o600});await rename(temporary,destination);}finally{await unlink(temporary).catch(()=>{});}return report;}
