// Same-lock server counter timestamps. Client request bounds remain private observations.
import assert from "node:assert/strict";
export const CLOCK_LIMITS = Object.freeze({observations:61,driftMicros:1000,httpQuantizationMicros:1000,elapsedRoundingMicros:2});
export const CLOCK_COUNTERS = Object.freeze(["frames_sent","packets_sent","deadline_misses","send_errors"]);
const FAILURES = ["clock_metadata_failures","clock_bounds_failures","clock_monotonic_failures","clock_counter_failures","clock_elapsed_failures","clock_coherence_failures","clock_count_failures"];
export const CLOCK_NUMERIC_FIELDS = Object.freeze(["clock_source_code","clock_observations","clock_server_samples","clock_validated_samples","clock_validation_passed","clock_failure_code",...FAILURES,"clock_max_drift_micros","clock_max_http_duration_micros","clock_max_http_midpoint_offset_micros","clock_max_interval_midpoint_difference_micros","clock_drift_limit_micros","clock_http_quantization_micros","clock_elapsed_rounding_micros"]);
const uint = n => Number.isSafeInteger(n)&&n>=0;
export function emptyClockSummary() {
 return {...Object.fromEntries(CLOCK_NUMERIC_FIELDS.map(k=>[k,0])),clock_drift_limit_micros:CLOCK_LIMITS.driftMicros,clock_http_quantization_micros:CLOCK_LIMITS.httpQuantizationMicros,clock_elapsed_rounding_micros:CLOCK_LIMITS.elapsedRoundingMicros};
}
export function validateClockSummary(r) {
 assert.deepEqual(Object.keys(r).sort(),CLOCK_NUMERIC_FIELDS.slice().sort(),"closed clock summary");
 for(const n of Object.values(r))assert(uint(n),"unsigned clock metric");
 assert([0,1].includes(r.clock_validation_passed)&&[0,1].includes(r.clock_source_code));
 assert(r.clock_failure_code===0||r.clock_failure_code>=2&&r.clock_failure_code<=8);
 assert(r.clock_drift_limit_micros===1000&&r.clock_http_quantization_micros===1000&&r.clock_elapsed_rounding_micros===2);
 const failures=FAILURES.reduce((n,k)=>n+r[k],0);
 assert(failures<=1&&Number(failures>0)===Number(r.clock_failure_code>0));
 if(failures)assert(r[FAILURES[r.clock_failure_code-2]]===1);
 assert(r.clock_observations<=62&&r.clock_server_samples<=r.clock_observations&&r.clock_validated_samples<=r.clock_server_samples);
 assert(r.clock_validated_samples+failures===r.clock_observations);
 assert(r.clock_source_code===Number(r.clock_server_samples>0));
 if(r.clock_coherence_failures)assert(r.clock_max_drift_micros>r.clock_drift_limit_micros);
 if(!r.clock_coherence_failures)assert(r.clock_max_drift_micros<=r.clock_drift_limit_micros);
 assert(r.clock_validation_passed===Number(r.clock_observations===61&&r.clock_server_samples===61&&r.clock_validated_samples===61&&!failures));
 return r;
}
// Exact server bounds are private. Millisecond-only callers retain their original conversion.
export function diagnosticWindowBounds(window) {
 const {startedAt,endedAt}=window??{};
 if(![startedAt,endedAt].every(n=>typeof n==="number"&&Number.isFinite(n)&&n>=0)||endedAt<=startedAt)return null;
 if(Object.hasOwn(window,"startedAtMicros")||Object.hasOwn(window,"endedAtMicros")) {
  const {startedAtMicros,endedAtMicros}=window;
  if(!uint(startedAtMicros)||!uint(endedAtMicros)||endedAtMicros<=startedAtMicros||endedAtMicros-startedAtMicros>400_000_000||startedAt!==startedAtMicros/1000||endedAt!==endedAtMicros/1000)return null;
  return {startedAtNanos:BigInt(startedAtMicros)*1000n,endedAtNanos:BigInt(endedAtMicros)*1000n,seconds:(endedAtMicros-startedAtMicros)/1e6};
 }
 if(endedAt-startedAt>400_000)return null;
 const nanos=n=>BigInt(Math.trunc(n))*1_000_000n+BigInt(Math.round((n-Math.trunc(n))*1_000_000));
 return {startedAtNanos:nanos(startedAt),endedAtNanos:nanos(endedAt),seconds:(endedAt-startedAt)/1000};
}
export async function capturePerformanceSample(api, clocks={epoch:()=>Date.now(),monotonic:()=>performance.now()}) {
 const diagnosticsMonotonicStartMillis=clocks.monotonic(),diagnosticsUtcStartMillis=clocks.epoch();
 const diagnostics=await api.request("GET","/api/v2/diagnostics/performance");
 const diagnosticsUtcEndMillis=clocks.epoch(),diagnosticsMonotonicEndMillis=clocks.monotonic();
 return {diagnostics,diagnosticsUtcStartMillis,diagnosticsUtcEndMillis,diagnosticsMonotonicStartMillis,diagnosticsMonotonicEndMillis};
}
export class ServerCounterClock {
 constructor(){this.status=emptyClockSummary();this.samples=[];}
 reject(code,message){this.status.clock_failure_code=code;this.status[FAILURES[code-2]]=1;throw new Error(message);}
 observe(record) {
  assert(this.status.clock_failure_code===0,"server clock already failed");
  this.status.clock_observations++;
  if(this.status.clock_observations>61)this.reject(8,"too many server clock observations");
  const sample=record?.diagnostics?.output_sample;
  if(!sample||Array.isArray(sample)||Object.keys(sample).sort().join(",")!=="monotonic_micros,sampled_at_epoch_micros"||!uint(sample.sampled_at_epoch_micros)||!uint(sample.monotonic_micros))this.reject(2,"missing or malformed server counter timestamp");
  this.status.clock_server_samples++;this.status.clock_source_code=1;
  const {diagnosticsUtcStartMillis:utcStart,diagnosticsUtcEndMillis:utcEnd,diagnosticsMonotonicStartMillis:monoStart,diagnosticsMonotonicEndMillis:monoEnd}=record;
  if(!uint(utcStart)||!uint(utcEnd)||utcEnd<utcStart||![monoStart,monoEnd].every(n=>typeof n==="number"&&Number.isFinite(n)&&n>=0)||monoEnd<monoStart)this.reject(2,"invalid client HTTP clock bounds");
  // Date.now() returns a whole millisecond: its response reading denotes [t,t+1ms).
  if(sample.sampled_at_epoch_micros<utcStart*1000||sample.sampled_at_epoch_micros>=(utcEnd*1000+1000))this.reject(3,"server timestamp outside HTTP UTC bounds");
  const previous=this.samples.at(-1),first=this.samples[0];
  if(previous&&(sample.monotonic_micros<=previous.monotonic_micros||sample.sampled_at_epoch_micros<=previous.sampled_at_epoch_micros||monoStart<previous.clientMonotonicEndMillis||utcStart<previous.clientUtcStartMillis))this.reject(4,"nonmonotonic server/client observations");
  const counters=Object.fromEntries(CLOCK_COUNTERS.map(k=>[k,record.diagnostics.output?.[k]]));
  if(!Object.values(counters).every(uint)||previous&&CLOCK_COUNTERS.some(k=>counters[k]<previous.counters[k]))this.reject(5,"invalid or regressing output counter");
  const elapsedMicros=previous?sample.monotonic_micros-previous.monotonic_micros:0;
  // Comparisons describe the replaced observer approximation only; they never gate cadence.
  // UTC midpoint comparison retains Date.now()'s whole-millisecond floor uncertainty.
  const comparisons={
   clock_max_http_duration_micros:Math.ceil((monoEnd-monoStart)*1000),
   clock_max_http_midpoint_offset_micros:Math.ceil(Math.abs((sample.sampled_at_epoch_micros-utcStart*1000)-(utcEnd-utcStart)*500)),
   clock_max_interval_midpoint_difference_micros:previous?Math.ceil(Math.abs(elapsedMicros-((monoStart+monoEnd)-(previous.clientMonotonicStartMillis+previous.clientMonotonicEndMillis))*500)):0,
  };
  if(!Object.values(comparisons).every(uint))this.reject(2,"unsafe client clock comparison");
  for(const [key,value]of Object.entries(comparisons))this.status[key]=Math.max(this.status[key],value);
  if(previous) {
   const lower=(monoStart-previous.clientMonotonicEndMillis)*1000-2,upper=(monoEnd-previous.clientMonotonicStartMillis)*1000+2;
   if(elapsedMicros<lower||elapsedMicros>upper)this.reject(6,"server elapsed time outside client request envelope");
  }
  const drift=first?Math.abs((sample.sampled_at_epoch_micros-first.sampled_at_epoch_micros)-(sample.monotonic_micros-first.monotonic_micros)):0;
  this.status.clock_max_drift_micros=Math.max(this.status.clock_max_drift_micros,drift);
  if(drift>1000)this.reject(7,"server UTC and monotonic clocks diverged");
  const value={...sample,counters,clientUtcStartMillis:utcStart,clientUtcEndMillis:utcEnd,clientMonotonicStartMillis:monoStart,clientMonotonicEndMillis:monoEnd};
  this.samples.push(value);this.status.clock_validated_samples++;
  return {elapsedSeconds:previous?elapsedMicros/1e6:null};
 }
 summary(){const r={...this.status};r.clock_validation_passed=Number(r.clock_observations===61&&r.clock_validated_samples===61&&!r.clock_failure_code);return validateClockSummary(r);}
 window(){assert(this.summary().clock_validation_passed===1,"server clock coverage incomplete");const startedAtMicros=this.samples[0].sampled_at_epoch_micros,endedAtMicros=this.samples.at(-1).sampled_at_epoch_micros;return {startedAt:startedAtMicros/1000,endedAt:endedAtMicros/1000,startedAtMicros,endedAtMicros};}
 elapsedSeconds(){assert(this.summary().clock_validation_passed===1);return (this.samples.at(-1).monotonic_micros-this.samples[0].monotonic_micros)/1e6;}
 delta(name){assert(CLOCK_COUNTERS.includes(name));assert(this.summary().clock_validation_passed===1);return this.samples.at(-1).counters[name]-this.samples[0].counters[name];}
}
