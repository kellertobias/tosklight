import test from "node:test";
import assert from "node:assert/strict";
import { mkdtemp,writeFile,readFile,stat,readdir,rm,open } from "node:fs/promises";
import path from "node:path";
import { artifactPaths } from "./artifact-paths.mjs";
import { PHASE_FIELDS } from "./desktop-performance-576-phases.mjs";
import { APPLICATION_FIELDS,TICK_METRIC_FIELDS,TICK_TRACE_LIMITS,parseTickTraceDiagnostics,validateTickTraceDiagnostics,readTickTraceDiagnostics,writeTickTraceDiagnostics } from "./desktop-performance-576-tick-trace.mjs";
const epoch=Date.parse("2026-10-17T10:00:00Z"),window={startedAt:epoch,endedAt:epoch+60_000};
function line(seq,message,payload,offset=100) {return `${new Date(epoch+offset).toISOString()} INFO output_tick{tick_seq=${seq}}: light_output::scheduler: ${message} ${payload}\n`;}
function tick(seq=1,{at=100,miss=false,pathCode=0,nested=pathCode!==1,accepted=pathCode===0}={}) {
 const app=APPLICATION_FIELDS.map((k,i)=>`${k}=${pathCode===3&&!nested&&k==="ordered_operation_micros"?0:1000+i}`).join(" "),phase=PHASE_FIELDS.map((k,i)=>`${k}=${10+i}`).join(" ");
 return [
  ...(nested?[line(seq,"bounded live output nested phase sample",`generation=Some(123) show_revision=Some(99) sampled_at_millis=Some(${epoch+at}) accepted=${accepted} ${phase} family_diagnostics_schema=1 family_stage_calls=[1, 0, 1] family_counts=[22, 4, 0]`,at-2)]:[]),
  line(seq,"output tick application sections",`${app} retained=${pathCode===1||pathCode===2} path_code=${pathCode}`,at-1),
  line(seq,"bounded output scheduler tick sample",`schema_version=1 interval_micros=25000 tick_micros=${miss?60000:14000} scheduled_tick=1 result_ok=${Number(pathCode!==3)} packets=${pathCode===3?0:16}`,at),
  line(seq,"bounded output scheduler deadline decision",`schema_version=1 interval_micros=25000 lateness_micros=${miss?30000:0} missed_deadline=${Number(miss)} decision_code=${miss?0:1}`,at+1),
  line(seq,"bounded output scheduler wait result",`schema_version=1 resume_reason=${miss?0:1}`,at+2),
 ].join("");
}
test("full tick identities join all records and retain all-versus-actual-miss distributions",()=>{
 const report=parseTickTraceDiagnostics(tick(1)+tick(2,{at:200,miss:true})+tick(3,{at:300}),window,{healthDeadlineMisses:2,completedFrames:4});
 assert.deepEqual([report.available,report.capture_complete,report.parse_complete,report.join_complete],[1,1,1,1]);
 assert.deepEqual(report.groups.map(g=>[g.id,g.ticks,g.nested_ticks]),[[0,3,3],[1,1,1]]);
 assert.equal(report.groups[0].metrics.length,33);assert.equal(report.groups[0].metrics[30].mean_micros,(14000+60000+14000)/3);
 assert.equal(report.groups[1].metrics[32].mean_micros,30000);assert.equal(report.health_deadline_delta,-1);assert.equal(report.completed_frame_delta,-1);
 assert.equal(report.nested_accepted,3);assert.equal(report.nested_rejected,0);
 assert.deepEqual(validateTickTraceDiagnostics(JSON.parse(JSON.stringify(report))),report);
 assert(!JSON.stringify(report).includes("2026-"));assert(!JSON.stringify(report).includes("tick_seq="));assert(!JSON.stringify(report).includes("private"));
});
test("window uses tick completion timestamp, joins later deadline/wait and never guesses nearest timestamps",()=>{
 const text=tick(1,{at:-1})+tick(2,{at:0})+tick(3,{at:60_000,miss:true})+tick(4,{at:60_001});
 const report=parseTickTraceDiagnostics(text,window);assert.equal(report.available,1);assert.equal(report.selected_ticks,2);assert.equal(report.ticks_before_window,1);assert.equal(report.ticks_after_window,1);assert.equal(report.groups[1].ticks,1);
 const broken=text.replace("output_tick{tick_seq=3}: light_output::scheduler: bounded output scheduler wait result","output_tick{tick_seq=9}: light_output::scheduler: bounded output scheduler wait result");
 const fail=parseTickTraceDiagnostics(broken,window);assert.equal(fail.available,0);assert.equal(fail.selected_missing_wait,1);assert.equal(fail.orphan_records,1);
});
test("retained/error paths may omit nested records; normal paths require accepted nested samples",()=>{
 const report=parseTickTraceDiagnostics(tick(1,{pathCode:1})+tick(2,{at:200,pathCode:2,nested:true,accepted:false})+tick(3,{at:300,pathCode:3}),window);
 assert.equal(report.available,1);assert.equal(report.retained_ticks,2);assert.equal(report.error_ticks,1);assert.equal(report.nested_rejected,2);assert.equal(report.successful_ticks,2);assert.equal(report.groups[0].metrics[0].count,2);
 const missing=parseTickTraceDiagnostics(tick(1,{nested:false}),window);assert.equal(missing.selected_missing_nested,1);assert.equal(missing.available,0);
 const rejected=parseTickTraceDiagnostics(tick(1,{accepted:false}),window);assert.equal(rejected.metadata_mismatches,1);assert.equal(rejected.available,0);
});
test("missing domain/application records, duplicate identity and sequence gaps are explicit failures",()=>{
 for(const message of ["bounded output scheduler deadline decision","bounded output scheduler wait result","output tick application sections"]) {
  const text=tick().split("\n").filter(l=>!l.includes(message)).join("\n");const r=parseTickTraceDiagnostics(text,window);assert.equal(r.available,0);assert.equal(r.join_complete,0);
 }
 const duplicate=parseTickTraceDiagnostics(tick()+tick().split("\n").find(l=>l.includes("tick sample"))+"\n",window);assert.equal(duplicate.duplicate_records,1);assert.equal(duplicate.selected_duplicate_records,1);assert.equal(duplicate.available,0);
 const gap=parseTickTraceDiagnostics(tick(1)+tick(3,{at:300}),window);assert.equal(gap.selected_tick_seq_span,3);assert.equal(gap.selected_missing_tick,1);assert.equal(gap.available,0);
});
test("bounded u64 tick identity stays exact without exposing identifiers publicly",()=>{
 const first=16383n;const r=parseTickTraceDiagnostics(tick(first)+tick(first+1n,{at:200}),window);assert.equal(r.available,1);assert.equal(r.selected_tick_seq_span,2);assert(!JSON.stringify(r).includes(first.toString()));
 const bad=parseTickTraceDiagnostics(tick(16385n),window);assert.equal(bad.available,0);assert(bad.invalid_records>0);
});
test("unknown/missing/duplicate/nonfinite fields and unknown spans fail closed",()=>{
 for(const mutate of [
  s=>s.replace("tick_micros=14000","tick_micros=NaN"),s=>s.replace("tick_micros=14000","tick_micros=-1"),s=>s.replace("tick_micros=14000","tick_micros=1.5"),
  s=>s.replace("tick_micros=14000","tick_micros=Infinity"),s=>s.replace("tick_micros=14000",`tick_micros=${TICK_TRACE_LIMITS.micros+1}`),
  s=>s.replace("tick_micros=14000","tick_micros=14000 private_name=secret"),s=>s.replace("tick_micros=14000","tick_micros=14000 tick_micros=2"),
  s=>s.replace("tick_micros=14000 ",""),s=>s.replaceAll("output_tick{tick_seq=1}","wrong_span{tick_seq=1}"),
  s=>s.replace("schema_version=1","schema_version=2"),s=>s.replace("2026-10-17T10:00:00.100Z","2026-02-30T10:00:00.100Z"),
 ]) {const r=parseTickTraceDiagnostics(mutate(tick()),window);assert.equal(r.available,0);assert(r.invalid_records>0);assert(!JSON.stringify(r).includes("secret"));}
});
test("contradictory application, scheduler interval and decision metadata cannot claim joined coverage",()=>{
 for(const mutate of [
  s=>s.replace("retained=false","retained=true"),s=>s.replace("result_ok=1","result_ok=0"),
  s=>s.replace("interval_micros=25000 lateness_micros","interval_micros=16666 lateness_micros"),
  s=>s.replace("resume_reason=1","resume_reason=0"),s=>s.replace("missed_deadline=0","missed_deadline=1"),
 ]) {const r=parseTickTraceDiagnostics(mutate(tick()),window);assert.equal(r.available,0);assert.equal(r.metadata_mismatches,1);}
});
test("unrelated lines and terminal orphan after the selected window do not invent missing selected ticks",()=>{
 const orphan=line(2,"bounded output scheduler wait result","schema_version=1 resume_reason=2",60_100);
 const r=parseTickTraceDiagnostics("unrelated private log\n"+tick()+orphan,window);assert.equal(r.available,1);assert.equal(r.orphan_records,1);assert.equal(r.selected_ticks,1);
});
test("truncation, cap marker, missing window and byte/line/per-line caps are explicit unavailable reports",()=>{
 const trunc=parseTickTraceDiagnostics(tick().trimEnd(),window);assert.equal(trunc.read_status,8);assert.equal(trunc.capture_complete,0);assert.equal(trunc.available,0);
 const cap=parseTickTraceDiagnostics(tick()+`${new Date(epoch+200).toISOString()} INFO bounded output scheduler trace cap reached schema_version=1 admitted_ticks=16384\n`,window);assert.equal(cap.tick_cap_records,1);assert.equal(cap.read_status,9);assert.equal(cap.available,0);
 for(const [text,status] of [["x".repeat(TICK_TRACE_LIMITS.bytes+1),2],["\n".repeat(TICK_TRACE_LIMITS.lines+1),3],["x".repeat(TICK_TRACE_LIMITS.lineBytes+1)+"\n",4]]) {const r=parseTickTraceDiagnostics(text,window);assert.equal(r.read_status,status);assert.equal(r.available,0);}
 assert.equal(parseTickTraceDiagnostics(tick(),null).read_status,7);assert.equal(parseTickTraceDiagnostics("",window).available,0);
});
test("closed numeric validation rejects privacy, drift, fabricated completeness and nonfinite durations",()=>{
 const good=parseTickTraceDiagnostics(tick(),window);
 for(const mutate of [r=>r.pid=1,r=>r.groups[0].metrics[0].symbol="secret",r=>r.groups[0].metrics[0].count=2,r=>r.groups[0].metrics[0].mean_micros=Infinity,r=>r.selected_ticks=2,r=>r.metadata_mismatches=1,r=>r.nested_accepted=0,r=>r.available=0]) {const bad=structuredClone(good);mutate(bad);assert.throws(()=>validateTickTraceDiagnostics(bad));}
});
test("bounded read and atomic mode0600 output preserve unavailable status without leaking raw data",async()=>{
 const dir=await mkdtemp(path.join(artifactPaths.tmp,"tl775-tick-trace-"));
 try {
  const raw=path.join(dir,"private.log"),out=path.join(dir,"tick-trace-numeric.json");await writeFile(raw,tick());const r=await writeTickTraceDiagnostics(out,raw,window);assert.equal(r.available,1);assert.equal((await stat(out)).mode&0o777,0o600);assert.deepEqual(validateTickTraceDiagnostics(JSON.parse(await readFile(out,"utf8"))),r);
  const missing=await readTickTraceDiagnostics(path.join(dir,"missing"),window);assert.equal(missing.read_status,1);assert.equal(missing.log_available,0);
  const h=await open(raw,"w");await h.truncate(TICK_TRACE_LIMITS.bytes+1);await h.close();assert.equal((await readTickTraceDiagnostics(raw,window)).read_status,2);
  assert.deepEqual((await readdir(dir)).sort(),["private.log","tick-trace-numeric.json"]);
 }finally{await rm(dir,{recursive:true,force:true});}
});
test("fixed metric IDs reuse all25 existing nested fields and add only5application and3domain durations",()=>{
 assert.equal(TICK_METRIC_FIELDS.length,33);assert.deepEqual(TICK_METRIC_FIELDS.slice(0,25),PHASE_FIELDS);assert.deepEqual(TICK_METRIC_FIELDS.slice(25,30),APPLICATION_FIELDS);assert.deepEqual(TICK_METRIC_FIELDS.slice(30),["tick_micros","interval_micros","lateness_micros"]);
});

test("real retained-error and delivery-error paths retain acceptedness without inventing missing work",()=>{
 const retainedError=tick(1,{pathCode:3,nested:false}).replace("retained=false","retained=true");
 assert.equal(parseTickTraceDiagnostics(retainedError,window).available,1);
 const deliveryError=tick(1,{pathCode:3,nested:true,accepted:true});
 const delivery=parseTickTraceDiagnostics(deliveryError,window);assert.equal(delivery.available,1);assert.equal(delivery.nested_accepted,1);assert.equal(delivery.error_ticks,1);
 const missingError=tick(1,{pathCode:3,nested:false});assert.equal(parseTickTraceDiagnostics(missingError,window).selected_missing_nested,1);
 const missingStale=tick(1,{pathCode:2,nested:false});assert.equal(parseTickTraceDiagnostics(missingStale,window).selected_missing_nested,1);
 const unexpectedActivation=tick(1,{pathCode:1,nested:true,accepted:false});assert.equal(parseTickTraceDiagnostics(unexpectedActivation,window).metadata_mismatches,1);
});
test("submicrosecond overdue decision remains immediate despite a zero rounded lateness",()=>{
 const immediate=tick(1).replace("decision_code=1","decision_code=0").replace("resume_reason=1","resume_reason=0");
 assert.equal(parseTickTraceDiagnostics(immediate,window).available,1);
});
test("causal event timestamp order cannot be swapped while preserving the tick identity",()=>{
 const text=tick();
 const between=text.replace(new Date(epoch+98).toISOString(),"2026-10-17T10:00:00.099500Z");
 assert.equal(parseTickTraceDiagnostics(between,window).metadata_mismatches,1);
 for(const [oldOffset,newOffset] of [[99,101],[98,101],[101,99],[102,100]]) {
  const changed=text.replace(new Date(epoch+oldOffset).toISOString(),new Date(epoch+newOffset).toISOString());
  const r=parseTickTraceDiagnostics(changed,window);assert.equal(r.metadata_mismatches,1);assert.equal(r.available,0);
 }
});
test("missing/unavailable trace retains nonzero health context without inventing a comparable delta",async()=>{
 const observed={healthDeadlineMisses:11,completedFrames:2493};
 for(const r of [await readTickTraceDiagnostics(null,window,observed),await readTickTraceDiagnostics(null,null,observed),parseTickTraceDiagnostics("",window,observed)]) {
  assert.equal(r.health_deadline_misses,11);assert.equal(r.completed_frames,2493);assert.equal(r.available,0);assert.equal(r.health_deadline_comparison_available,0);assert.equal(r.completed_frames_comparison_available,0);assert.equal(r.health_deadline_delta,0);assert.equal(r.completed_frame_delta,0);
 }
});
test("exact domain and application event fields bind the current Rust emitters and tested span format",async()=>{
 const domain=await readFile(new URL("../crates/light/domain/output/src/scheduler/diagnostics.rs",import.meta.url),"utf8");
 const app=await readFile(new URL("../crates/light/adapters/headless/src/runtime/output_scheduler/slow_phase_trace.rs",import.meta.url),"utf8");
 for(const [source,message,expected] of [
  [domain,"bounded output scheduler tick sample",["schema_version","interval_micros","tick_micros","scheduled_tick","result_ok","packets"]],
  [domain,"bounded output scheduler deadline decision",["schema_version","interval_micros","lateness_micros","missed_deadline","decision_code"]],
  [domain,"bounded output scheduler wait result",["schema_version","resume_reason"]],
  [app,"output tick application sections",[...APPLICATION_FIELDS,"retained","path_code"]],
 ]) {
  const end=source.indexOf(`"${message}"`);assert(end>0);const begin=source.lastIndexOf("tracing::info!(",end);assert(begin>=0);
  const block=source.slice(begin,end),fields=[...block.matchAll(/(?:^|\n)\s*([a-z_]+)(?:\s*=|\s*,)/gu)].map(m=>m[1]);assert.deepEqual(fields,expected);
 }
 const fixture=await readFile(new URL("../crates/light/domain/output/src/scheduler/diagnostics_tests.rs",import.meta.url),"utf8");assert(fixture.includes("output_tick{tick_seq=1}"));assert(domain.includes('tracing::info_span!("output_tick", tick_seq = self.admitted)'));
});

test("zero-rounded ordered errors still require nested coverage and retained errors cannot claim accepted output",()=>{
 const missing=parseTickTraceDiagnostics(tick(1,{pathCode:3,nested:false}),window);assert.equal(missing.selected_missing_nested,1);assert.equal(missing.available,0);
 const acceptedRetained=tick(1,{pathCode:3,nested:true,accepted:true}).replace("retained=false","retained=true");assert.equal(parseTickTraceDiagnostics(acceptedRetained,window).metadata_mismatches,1);
 const failedWithPackets=tick(1,{pathCode:3,nested:true}).replace("packets=0","packets=16");assert.equal(parseTickTraceDiagnostics(failedWithPackets,window).metadata_mismatches,1);
});
test("closed producer invariants reject forged read status, empty joined metrics and false join flags",()=>{
 const good=parseTickTraceDiagnostics(tick(),window);
 for(const mutate of [r=>r.read_status=2,r=>r.groups[0].metrics[0]={id:0,count:0,min_micros:0,mean_micros:0,p50_micros:0,p95_micros:0,max_micros:0},r=>{r.join_complete=0;r.available=0;},r=>r.record_kinds[0].count++]) {const bad=structuredClone(good);mutate(bad);assert.throws(()=>validateTickTraceDiagnostics(bad));}
});

test("incomplete diagnostics cannot forge missing metric counts, cap state or fractional health counters",()=>{
 const good=parseTickTraceDiagnostics(tick(),window,{healthDeadlineMisses:1,completedFrames:1});
 for(const mutate of [r=>r.health_deadline_misses=.5,r=>r.completed_frames=.5,r=>r.successful_ticks=.5,r=>r.tick_cap_records=1,r=>r.groups[0].metrics[0].p95_micros=10.5]) {const bad=structuredClone(good);mutate(bad);assert.throws(()=>validateTickTraceDiagnostics(bad));}
 const incomplete=parseTickTraceDiagnostics(tick().split("\n").filter(l=>!l.includes("deadline decision")).join("\n"),window);
 assert.equal(incomplete.join_complete,0);
 for(const id of [0,25,30,31]) {const bad=structuredClone(incomplete);bad.groups[0].metrics[id]={id,count:0,min_micros:0,mean_micros:0,p50_micros:0,p95_micros:0,max_micros:0};assert.throws(()=>validateTickTraceDiagnostics(bad));}
});
