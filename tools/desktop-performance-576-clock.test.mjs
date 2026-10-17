import test from "node:test";
import assert from "node:assert/strict";
import { capturePerformanceSample,ServerCounterClock,validateClockSummary,emptyClockSummary,CLOCK_NUMERIC_FIELDS } from "./desktop-performance-576-clock.mjs";
const epoch=1_792_231_200_000;
function sample(i,{utcOffset=0,monoOffset=0,requestStartOffset=-2,requestEndOffset=2,serverOffset=500}={}) {
 const utc=epoch+i*1000;
 return {diagnosticsUtcStartMillis:utc+requestStartOffset,diagnosticsUtcEndMillis:utc+requestEndOffset,diagnosticsMonotonicStartMillis:100+i*1000+requestStartOffset,diagnosticsMonotonicEndMillis:100+i*1000+requestEndOffset,diagnostics:{output_sample:{sampled_at_epoch_micros:utc*1000+serverOffset+utcOffset,monotonic_micros:i*1_000_000+500+monoOffset},output:{frames_sent:10+i*40,packets_sent:160+i*640,deadline_misses:0,send_errors:0}}};
}
function complete(make=i=>sample(i)){const c=new ServerCounterClock();for(let i=0;i<=60;i++)c.observe(make(i));return c;}
test("61 same-lock server samples drive exact cadence/window and fixed numeric clock evidence",()=>{
 const c=complete();assert.equal(c.summary().clock_validation_passed,1);assert.equal(c.summary().clock_source_code,1);assert.equal(c.summary().clock_server_samples,61);assert.equal(c.elapsedSeconds(),60);assert.equal(c.delta("frames_sent"),2400);assert.deepEqual(c.window(),{startedAt:epoch+.5,endedAt:epoch+60_000+.5});assert.deepEqual(validateClockSummary(JSON.parse(JSON.stringify(c.summary()))),c.summary());
 assert(!JSON.stringify(c.summary()).includes(String(epoch)));assert.deepEqual(Object.keys(c.summary()).sort(),CLOCK_NUMERIC_FIELDS.slice().sort());
});
test("asymmetric slow HTTP responses cannot skew server cadence or the packet window",()=>{
 const c=complete(i=>sample(i,{requestStartOffset:i%2?-200:-5,requestEndOffset:i%2?1:250}));assert.equal(c.elapsedSeconds(),60);assert.equal(c.delta("frames_sent")/c.elapsedSeconds(),40);assert.equal(c.summary().clock_validation_passed,1);
});
test("Date.now response floor admits actual microseconds in its one-millisecond interval only",()=>{
 const c=new ServerCounterClock();c.observe(sample(0,{requestStartOffset:0,requestEndOffset:0,serverOffset:999}));assert.equal(c.summary().clock_validated_samples,1);
 for(const offset of [-1,1000]) {const bad=new ServerCounterClock();assert.throws(()=>bad.observe(sample(0,{requestStartOffset:0,requestEndOffset:0,serverOffset:offset})),/outside HTTP/);assert.equal(bad.summary().clock_bounds_failures,1);}
});
test("missing, malformed, unsafe and unexpected metadata fail closed with partial counts",()=>{
 for(const mutate of [r=>delete r.diagnostics.output_sample,r=>r.diagnostics.output_sample.monotonic_micros=NaN,r=>r.diagnostics.output_sample.monotonic_micros=-1,r=>r.diagnostics.output_sample.sampled_at_epoch_micros=Number.MAX_SAFE_INTEGER+1,r=>r.diagnostics.output_sample.private_id=42]) {
  const c=new ServerCounterClock();c.observe(sample(0));const next=sample(1);mutate(next);assert.throws(()=>c.observe(next));const status=c.summary();assert.equal(status.clock_observations,2);assert.equal(status.clock_validated_samples,1);assert.equal(status.clock_metadata_failures,1);assert.equal(status.clock_validation_passed,0);
 }
});
test("all four cumulative counters must remain safe and nondecreasing at every observation",()=>{
 for(const name of ["frames_sent","packets_sent","deadline_misses","send_errors"])for(const value of [-1,NaN,Infinity,1.5,Number.MAX_SAFE_INTEGER+1]) {const c=new ServerCounterClock();c.observe(sample(0));const next=sample(1);next.diagnostics.output[name]=value;assert.throws(()=>c.observe(next));assert.equal(c.summary().clock_counter_failures,1);}
 const c=new ServerCounterClock();const first=sample(0);first.diagnostics.output.deadline_misses=5;c.observe(first);assert.throws(()=>c.observe(sample(1)),/counter/);assert.equal(c.summary().clock_counter_failures,1);
});
test("server clock equality/regression and client request clock reversal are rejected",()=>{
 for(const mutate of [r=>r.diagnostics.output_sample.monotonic_micros=500,r=>r.diagnostics.output_sample.sampled_at_epoch_micros=epoch*1000+500,r=>r.diagnosticsMonotonicStartMillis=99]) {
  const c=new ServerCounterClock();c.observe(sample(0));const next=sample(1);mutate(next);assert.throws(()=>c.observe(next));assert.equal(c.summary().clock_validation_passed,0);
 }
 const c=new ServerCounterClock();const bad=sample(0);bad.diagnosticsUtcEndMillis=bad.diagnosticsUtcStartMillis-1;assert.throws(()=>c.observe(bad));assert.equal(c.summary().clock_metadata_failures,1);
});
test("server elapsed time must fit both consecutive HTTP request envelopes with only two-microsecond rounding",()=>{
 for(const offset of [-5000,5000]) {const c=new ServerCounterClock();c.observe(sample(0));assert.throws(()=>c.observe(sample(1,{monoOffset:offset})),/request envelope/);assert.equal(c.summary().clock_elapsed_failures,1);}
 const boundary=new ServerCounterClock();boundary.observe(sample(0,{requestStartOffset:0,requestEndOffset:0}));boundary.observe(sample(1,{requestStartOffset:0,requestEndOffset:0,monoOffset:2}));assert.equal(boundary.summary().clock_validated_samples,2);
 const outside=new ServerCounterClock();outside.observe(sample(0,{requestStartOffset:0,requestEndOffset:0}));assert.throws(()=>outside.observe(sample(1,{requestStartOffset:0,requestEndOffset:0,monoOffset:3})),/request envelope/);
});
test("UTC-versus-monotonic drift is compared to the first sample, not hidden by per-step drift",()=>{
 const allowed=complete(i=>sample(i,{utcOffset:i*10}));assert.equal(allowed.summary().clock_max_drift_micros,600);
 const c=new ServerCounterClock();for(let i=0;i<=50;i++)c.observe(sample(i,{utcOffset:i*20}));assert.throws(()=>c.observe(sample(51,{utcOffset:1020})),/diverged/);assert.equal(c.summary().clock_max_drift_micros,1020);assert.equal(c.summary().clock_coherence_failures,1);
});
test("coverage is unavailable before exactly61 samples and rejects a 62nd sample",()=>{
 const c=new ServerCounterClock();assert.deepEqual(c.summary(),emptyClockSummary());assert.throws(()=>c.window());for(let i=0;i<60;i++)c.observe(sample(i));assert.equal(c.summary().clock_validation_passed,0);c.observe(sample(60));assert.equal(c.summary().clock_validation_passed,1);assert.throws(()=>c.observe(sample(61)),/too many/);assert.equal(c.summary().clock_count_failures,1);assert.equal(c.summary().clock_validation_passed,0);
});
test("capture retains exact UTC/performance request bounds around the unchanged diagnostics API",async()=>{
 const mono=[1,3],utc=[epoch,epoch+2];const response=sample(0).diagnostics;
 const calls=[];const result=await capturePerformanceSample({request:async(...args)=>{calls.push(args);return response;}},{epoch:()=>utc.shift(),monotonic:()=>mono.shift()});
 assert.deepEqual(calls,[["GET","/api/v2/diagnostics/performance"]]);assert.equal(result.diagnostics,response);assert.deepEqual([result.diagnosticsUtcStartMillis,result.diagnosticsUtcEndMillis,result.diagnosticsMonotonicStartMillis,result.diagnosticsMonotonicEndMillis],[epoch,epoch+2,1,3]);
});
test("numeric validation rejects forged coverage, unknown clock source, limits and privacy fields",()=>{
 const good=complete().summary();for(const mutate of [r=>r.pid=1,r=>r.clock_validation_passed=0,r=>r.clock_source_code=2,r=>r.clock_drift_limit_micros=1001,r=>r.clock_validated_samples=60,r=>r.clock_max_drift_micros=1001,r=>r.clock_failure_code=7,r=>r.clock_bounds_failures=1]){const bad=structuredClone(good);mutate(bad);assert.throws(()=>validateClockSummary(bad));}
});

test("actual runner uses server cadence while preserving separate process CPU intervals",async()=>{
 const {readFile}=await import("node:fs/promises");
 const source=await readFile(new URL("./run-desktop-performance-576-proof.mjs",import.meta.url),"utf8");
 const body=source.slice(source.indexOf("async function measureWindow("),source.indexOf("\nfunction apiClient("));
 const captures=Array.from({length:61},(_,i)=>sample(i,{requestStartOffset:i%2?-200:-5,requestEndOffset:i%2?1:250}));
 const cpuIntervals=[];let wall=0,index=0;
 const make=new Function("capturePerformanceSample","readLinuxProcessTree","processTreeResourceDelta","performance","setTimeout","DURATION_SECONDS","LINUX_PROCESS_OPTIONS","average","percentile",`${body}; return measureWindow;`);
 const measure=make(async()=>captures[index++],async()=>[{command:"light-headless",id:1}],(_a,_b,seconds)=>{cpuIntervals.push(seconds);return {cpuPercent:12,residentBytes:1024};},{now:()=>{wall+=5;return wall;}},fn=>fn(),60,{},values=>values.reduce((a,b)=>a+b,0)/values.length,values=>values[0]);
 const records=[],clock=new ServerCounterClock();const result=await measure({},123,"unrestricted",records,clock);
 assert.equal(index,61);assert.equal(result.counter_window_seconds,60);assert.equal(result.completed_frame_delta,2400);assert.equal(result.minimum_one_second_completed_hz,40);assert.equal(result.windows_below_target,0);assert.equal(result.clock_validation_passed,1);
 assert.equal(records.length,61);assert.equal(cpuIntervals.length,300);assert(cpuIntervals.every(seconds=>seconds===.01));assert(records.every(r=>r.diagnosticsUtcStartMillis!==undefined&&r.diagnosticsMonotonicEndMillis!==undefined&&r.processes.length===1));
 assert.deepEqual(clock.window(),{startedAt:epoch+.5,endedAt:epoch+60_000+.5});
 // Rejected raw observations survive; process evidence is not fabricated for the rejected row.
 index=0;delete captures[1].diagnostics.output_sample;const failed=[],badClock=new ServerCounterClock();await assert.rejects(()=>measure({},123,"unrestricted",failed,badClock),/timestamp/);assert.equal(failed.length,2);assert.equal(failed[1].processes,undefined);assert.equal(badClock.summary().clock_metadata_failures,1);
});

test("a different failure cannot disguise excessive UTC/monotonic drift",()=>{
 const c=new ServerCounterClock();c.observe(sample(0));const missing=sample(1);delete missing.diagnostics.output_sample;assert.throws(()=>c.observe(missing));const forged=c.summary();forged.clock_max_drift_micros=1001;assert.throws(()=>validateClockSummary(forged));
});
