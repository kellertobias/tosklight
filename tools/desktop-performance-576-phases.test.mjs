import test from "node:test";
import assert from "node:assert/strict";
import { readFile,mkdtemp,writeFile,rm,readdir,stat } from "node:fs/promises";
import path from "node:path";
import { artifactPaths } from "./artifact-paths.mjs";
import { PHASE_FIELDS,PHASE_LIMITS,parsePhaseDiagnostics,validatePhaseDiagnostics,readPhaseDiagnostics,writePhaseDiagnostics } from "./desktop-performance-576-phases.mjs";
const window={startedAt:Date.parse("2026-10-17T10:00:00.000Z"),endedAt:Date.parse("2026-10-17T10:01:00.000Z")};
function line(at=window.startedAt,value=10) {
  const fields=PHASE_FIELDS.map((field,id)=>`${field}=${value+id}`).join(" ");
  return `\x1b[2m${new Date(at).toISOString()}\x1b[0m \x1b[32m INFO\x1b[0m \x1b[2mlight_headless_runtime::private::scope:\x1b[0m bounded live output nested phase sample \x1b[3mgeneration\x1b[0m=Some(123456789) show_revision=Some(912) sampled_at_millis=Some(${at}) accepted=true ${fields} family_diagnostics_schema=1 family_stage_calls=[1, 0, 1] family_counts=[22, 4, 0]\n`;
}
test("exact25 duration codes bind current Rust source",async()=>{
  const source=await readFile(new URL("../crates/light/adapters/headless/src/runtime/output_scheduler/slow_phase_trace/live_phases.rs",import.meta.url),"utf8");
  const block=source.slice(source.indexOf("tracing::info!("),source.indexOf('"bounded live output nested phase sample"'));
  const fields=[...block.matchAll(/^\s*([a-z_]+_micros)\s*=/gmu)].map(row=>row[1]);
  assert.equal(fields.length,25);assert.deepEqual(PHASE_FIELDS,fields);
});
test("real ANSI tracing lines use inclusive actual wall-window boundaries and fixed numeric summary codes",()=>{
  const report=parsePhaseDiagnostics([line(window.startedAt-1),line(window.startedAt,10),line(window.startedAt+1000,20),line(window.endedAt,30),line(window.endedAt+1)].join(""),window);
  assert.deepEqual([report.available,report.parse_complete,report.samples_before_window,report.samples_in_window,report.samples_after_window],[1,1,1,3,1]);
  assert.deepEqual(report.metrics[0],{id:0,count:3,min_micros:10,mean_micros:20,p50_micros:20,p95_micros:30,max_micros:30});
  assert.equal(report.metrics[24].min_micros,34);
  assert(!JSON.stringify(report).includes("private"));assert(!JSON.stringify(report).includes("123456789"));assert(!JSON.stringify(report).includes("2026-"));
  assert.deepEqual(validatePhaseDiagnostics(JSON.parse(JSON.stringify(report))),report);
});
test("missing duplicate malformed nonfinite negative excessive and unknown duration fields are rejected without raw text",()=>{
  for(const transform of [
    text=>text.replace("capture_settle_micros=11 ",""),
    text=>text.replace("capture_settle_micros=11","capture_settle_micros=11 capture_settle_micros=12"),
    ...["NaN","Infinity","1e309","-1","1.5",String(PHASE_LIMITS.micros+1)].map(value=>text=>text.replace("capture_settle_micros=11",`capture_settle_micros=${value}`)),
    text=>text.replace("capture_settle_micros=11","secret_process=private-address capture_settle_micros=11"),
    text=>text.replace("capture_settle_micros=11","undocumented_duration_micros=55 capture_settle_micros=11"),
    text=>text.replace("family_diagnostics_schema=1","family_diagnostics_schema=2"),
  ]) {
    const report=parsePhaseDiagnostics(transform(line()),window);
    assert.equal(report.available,0);assert.equal(report.invalid_in_window,1);assert.equal(report.invalid_samples,1);assert.equal(report.samples_in_window,1);assert.equal(report.unlocalizable_samples,0);
    assert(report.metrics.every(row=>row.count===0));assert(!JSON.stringify(report).includes("private-address"));
  }
});
test("invalid timestamps are unlocalizable and invalid outside-window durations stay separately counted",()=>{
  const malformed=line().replace("2026-10-17T10:00:00.000Z","2026-02-30T10:00:00.000Z");
  const before=line(window.startedAt-1).replace("ordered_total_micros=10","ordered_total_micros=bad");
  const after=line(window.endedAt+1).replace("ordered_total_micros=10","ordered_total_micros=bad");
  const report=parsePhaseDiagnostics(malformed+before+line()+after,window);
  assert.equal(report.available,1);assert.equal(report.parse_complete,0);assert.equal(report.unlocalizable_samples,1);assert.equal(report.invalid_before_window,1);assert.equal(report.invalid_after_window,1);assert.equal(report.invalid_in_window,0);assert.equal(report.valid_samples_in_window,1);
});
test("empty missing unmeasured and partial logs cannot manufacture duration observations",async()=>{
  for(const text of ["","ordinary unrelated log\n"]) {const report=parsePhaseDiagnostics(text,window);assert.equal(report.available,0);assert.equal(report.log_available,1);assert.equal(report.capture_complete,1);}
  const partial=parsePhaseDiagnostics(line().trimEnd(),window);assert.equal(partial.read_status,8);assert.equal(partial.available,0);assert.equal(partial.capture_complete,0);
  const noWindow=parsePhaseDiagnostics(line(),null);assert.equal(noWindow.read_status,7);assert.equal(noWindow.available,0);
  const missing=await readPhaseDiagnostics(path.join(artifactPaths.tmp,"does-not-exist-phase-log"),window);assert.equal(missing.read_status,1);assert.equal(missing.log_available,0);
});
test("byte line sample and per-line caps produce bounded unavailable reports",()=>{
  for(const [text,status] of [["x".repeat(PHASE_LIMITS.bytes+1),2],["\n".repeat(PHASE_LIMITS.lines+1),3],["x".repeat(PHASE_LIMITS.lineBytes+1)+"\n",4],[line().repeat(PHASE_LIMITS.samples+1),5]]) {
    const report=parsePhaseDiagnostics(text,window);assert.equal(report.read_status,status);assert.equal(report.available,0);assert.equal(report.capture_complete,0);assert(report.metrics.every(row=>row.count===0));
  }
});
test("closed numeric validation rejects privacy fields count drift and forged availability",()=>{
  const good=parsePhaseDiagnostics(line(),window);
  for(const mutate of [r=>{r.pid=42;},r=>{r.metrics[0].symbol="private";},r=>{r.metrics[0].count++;},r=>{r.invalid_samples=1;},r=>{r.metrics[0].max_micros=Infinity;},r=>{r.available=0;}]) {const bad=structuredClone(good);mutate(bad);assert.throws(()=>validatePhaseDiagnostics(bad));}
});
test("bounded file reads and atomic0600 publication retain only closed numeric output",async()=>{
  const directory=await mkdtemp(path.join(artifactPaths.tmp,"tl775-phase-test-"));
  try {
    const raw=path.join(directory,"private.log"),output=path.join(directory,"phase-numeric.json");
    await writeFile(raw,line());await writePhaseDiagnostics(output,raw,window);
    const report=validatePhaseDiagnostics(JSON.parse(await readFile(output,"utf8")));assert.equal(report.available,1);assert.equal((await stat(output)).mode&0o777,0o600);
    await writePhaseDiagnostics(output,path.join(directory,"missing"),window);assert.equal(JSON.parse(await readFile(output,"utf8")).available,0);
    const handle=await (await import("node:fs/promises")).open(raw,"w");await handle.truncate(PHASE_LIMITS.bytes+1);await handle.close();
    const capped=await readPhaseDiagnostics(raw,window);assert.equal(capped.read_status,2);assert.equal(capped.available,0);
    assert.deepEqual((await readdir(directory)).sort(),["phase-numeric.json","private.log"]);
  } finally {await rm(directory,{recursive:true});}
});


test("nanosecond log fractions preserve inclusive boundaries without Date.parse millisecond truncation",()=>{
  const before=line().replace("2026-10-17T10:00:00.000Z","2026-10-17T09:59:59.999999999Z");
  const afterNano=line().replace("2026-10-17T10:00:00.000Z","2026-10-17T10:01:00.000000001Z");
  const afterMicro=line().replace("2026-10-17T10:00:00.000Z","2026-10-17T10:01:00.000001000Z");
  const report=parsePhaseDiagnostics(before+line(window.startedAt)+line(window.endedAt)+afterNano+afterMicro,window);
  assert.deepEqual([report.samples_before_window,report.samples_in_window,report.samples_after_window],[1,2,2]);
  const fractional={startedAt:window.startedAt+0.5,endedAt:window.endedAt+0.5};
  assert.equal(parsePhaseDiagnostics(line(),fractional).samples_before_window,1);
});
test("read status cannot claim a complete or available capped missing or unmeasured capture",()=>{
  const good=parsePhaseDiagnostics(line(),window);
  for(const status of [1,2,3,4,5,6,7,8]){const bad=structuredClone(good);bad.read_status=status;assert.throws(()=>validatePhaseDiagnostics(bad));}
  const missing=structuredClone(good);missing.log_available=0;assert.throws(()=>validatePhaseDiagnostics(missing));
});
