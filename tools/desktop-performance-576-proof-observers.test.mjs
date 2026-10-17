import test from "node:test";
import assert from "node:assert/strict";
import { readFile, mkdtemp, stat, readdir, rm } from "node:fs/promises";
import path from "node:path";
import { artifactPaths } from "./artifact-paths.mjs";
import { PacketAggregate, drainPacketRing, summarizeRamp, summarizeStageWindow, numericProof, validateNumericProof, verifyWorkloadFunctions, ProofCheckpoint, writeNumericProof, validateStageReady, waitStageReady } from "./desktop-performance-576-proof-observers.mjs";

const completedProgress = () => ({complete:1,current_stage_code:0,sheet_liveness_checked:1,sheet_liveness_passed:1,cleanup_complete:1});

function packet(sequence = 1, universe = 101) {
  return {protocol:"artnet",universe,sequence,slots:new Uint8Array(512),artnet:{payloadLength:512,protocolVersion:14}};
}
function stage() {
  const activity=[false,true].flatMap(followPreload => Array.from({length:63},(_,i) => ({paneId:followPreload?"main:preload-private-id":"main:live-private-id",attachmentId:followPreload?"2":"1",sequence:i+1,sourceFrame:i,followPreload,width:640,height:480,drawCalls:1,presentedEpochMicros:(99_000+i*1000)*1000})));
  return {schemaVersion:2,kind:"complete",measurementSurface:"packaged-tauri-native-stage",nativeStage:{frames:activity.map(row => ({...row,lane:row.followPreload?"preload":"normal"})),activity,heartbeats:[],observerOverflow:{activity:0,heartbeats:0,errors:0}}};
}
function staticPreloadStage() {
  const report=stage();
  const frame={...report.nativeStage.activity.find(row => row.followPreload),sourceFrame:7};
  report.nativeStage.activity=report.nativeStage.activity.filter(row => !row.followPreload).concat([frame]);
  report.nativeStage.frames=report.nativeStage.frames.filter(row => !row.followPreload).concat([{...frame,lane:"preload"}]);
  report.nativeStage.heartbeats=Array.from({length:125},(_,i) => ({paneId:frame.paneId,attachmentId:frame.attachmentId,sequence:i+1,epochMicros:(99_000+i*500)*1000,receivedEpochMicros:(99_001+i*500)*1000,lastPresentedSequence:frame.sequence,sourceFrame:frame.sourceFrame,followPreload:true,width:frame.width,height:frame.height,attached:true,visible:true,idleUnchanged:true,drawHealthy:true}));
  return report;
}
function result() {
  const resources = {cpu_average_percent:100,cpu_max_percent:120,peak_resident_bytes:1024};
  return {fixture_count:576,physical_instance_count:576,parameter_count:8064,universes:16,animated_attribute_count:5408,master_lane_count:20,counter_observation_count:61,counter_window_seconds:60,completed_frame_delta:2400,deadline_miss_delta:0,send_error_delta:0,minimum_one_second_completed_hz:40,average_completed_hz:40,windows_below_target:0,wall_monotonic_difference_ms:0,resources:{application_cpu_average_percent:400,application_cpu_max_percent:500,application_peak_resident_bytes:4096,server:resources,desktop_webview:resources,native_renderer:resources,desktop_host:resources},proof:{packets:{packet_count:38400,rows:Array.from({length:16},() => ({packets:2400,sequence_discontinuities:0,zero_sequence_packets:0}))},interaction:{accepted_commands:2,ramps:[{from_value:0,to_value:128,intermediate_values:100,target_delay_ms:3000,monotonic:1},{from_value:128,to_value:64,intermediate_values:60,target_delay_ms:3000,monotonic:1}]},stage:summarizeStageWindow(stage(),100_000,160_000)}};
}

test("executed focused workload declarations remain byte-identical to accepted canonical functions", async () => {
  const text = await readFile(new URL("./run-desktop-performance-576-proof.mjs",import.meta.url),"utf8");
  const functions = {};
  for (const name of ["prepareMixed","installDynamics","summarizePatch"]) {
    const begin = text.indexOf(`${name === "summarizePatch" ? "" : "async "}function ${name}(`);
    const end = text.indexOf(`\n${name === "installDynamics" ? "" : "async "}function `,begin+1);
    assert(begin > 0 && end > begin);
    functions[name] = new Function(`return (${text.slice(begin,end).trimEnd()});`)();
  }
  await verifyWorkloadFunctions(functions);
  await assert.rejects(() => verifyWorkloadFunctions({...functions,prepareMixed:async function prepareMixed() {}}),/executed workload drift/);
});
test("packet aggregate preserves all 16 universes and Art-Net rollover", () => {
  const aggregate = new PacketAggregate();
  for(let universe=101;universe<=116;universe++) for(const sequence of [254,255,1,2]) aggregate.add(packet(sequence,universe),1);
  assert.equal(aggregate.snapshot().packet_count,64);
  assert(aggregate.snapshot().rows.every(row => row.packets === 4 && row.sequence_discontinuities === 0));
});
test("packet discontinuity and disabled ordering remain separate numeric observations", () => {
  const aggregate = new PacketAggregate();
  for(const sequence of [1,3,0,200]) aggregate.add(packet(sequence),1);
  assert.deepEqual(aggregate.snapshot().rows[0],{packets:4,sequence_discontinuities:1,zero_sequence_packets:1});
});
test("packet cap, payload and foreign-universe failures cannot be silently counted", () => {
  const aggregate = new PacketAggregate();
  assert.throws(() => aggregate.add(packet(1,117),1),/unexpected/);
  assert.throws(() => aggregate.add({...packet(),slots:new Uint8Array(511)},1),/payload/);
  aggregate.count = 65536;
  assert.throws(() => aggregate.add(packet(),1),/cap/);
  assert.equal(aggregate.rows[0].packets,0);
});
test("fade requires intermediate values and approximately three seconds, in both directions", () => {
  for(const [from,to] of [[0,128],[128,64]]) {
    const rows = [{ms:900,value:from},...Array.from({length:31},(_,i) => ({ms:1000+i*100,value:Math.round(from+(to-from)*i/30)}))];
    assert.equal(summarizeRamp(rows,1000,from,to).target_delay_ms,3000);
    assert.throws(() => summarizeRamp([{ms:900,value:from},{ms:4000,value:to}],1000,from,to),/endpoint jump/);
    rows[20].value = from;
    assert.throws(() => summarizeRamp(rows,1000,from,to),/nonmonotonic/);
  }
});
test("Stage proves both lanes across exact measured window retrospectively", () => {
  const report = summarizeStageWindow(stage(),100_000,160_000);
  assert.equal(report.lanes.length,2); assert(report.lanes.every(row => row.bracketed === 1 && row.max_gap_ms === 1000));
});
test("Stage configuration labels alone, missing lane, edge gap or mismatched provenance retain failed numeric diagnostics", () => {
  assert.equal(summarizeStageWindow({kind:"complete",measurementSurface:"packaged-tauri-native-stage",activeUiSurfaces:["stage-native-live","stage-native-preload"]},100_000,160_000).validation_passed,0);
  for(const mutate of [
    report => {report.nativeStage.activity=report.nativeStage.activity.filter(row => !row.followPreload);},
    report => {report.nativeStage.activity=report.nativeStage.activity.filter(row => !(!row.followPreload && row.sequence > 10 && row.sequence < 20));},
    report => {report.nativeStage.activity[10].followPreload=true;},
    report => {report.nativeStage.activity=report.nativeStage.activity.filter(row => row.sequence >= 3);},
  ]) { const report=stage();mutate(report);const summary=summarizeStageWindow(report,100_000,160_000);assert.equal(summary.validation_passed,0);assert.equal(summary.report_available,1); }
});
test("public report roundtrip is closed, numeric, workload exact and free of private metadata", () => {
  const report = numericProof(result(),completedProgress()); assert.equal(report.all_gates_passed,1);
  assert.deepEqual(validateNumericProof(JSON.parse(JSON.stringify(report))),report);
  assert(!JSON.stringify(report).includes("private-id"));
  for(const mutate of [
    row => {row.pid=123;},row => {row.universes[0].destination="127.0.0.1";},
    row => {row.completed_frames=NaN;},row => {row.stage_lanes[0].paneId="secret";},
    row => {row.complete=0;},row => {row.fixture_records=575;},row => {row.deadline_misses=1;},
    row => {row.packet_count--;},row => {row.weighted_completed_hz=50;},
  ]) { const bad=structuredClone(report);mutate(bad);assert.throws(() => validateNumericProof(bad)); }
});
test("cadence deadline or sender errors fail proof without rewriting measured results", () => {
  for(const [name,value] of [["deadline_miss_delta",1],["send_error_delta",1],["minimum_one_second_completed_hz",37]]) {
    const measured = result();measured[name]=value;
    const report=numericProof(measured,completedProgress());assert.equal(report.complete,1);assert.equal(report.all_gates_passed,0);assert.equal(report.completed_frames,2400);
  }
});
test("incomplete failure has explicit unavailable zero values and cannot claim success", () => {
  const report=numericProof(null);assert.equal(report.complete,0);assert.equal(report.all_gates_passed,0);
  report.all_gates_passed=1;assert.throws(() => validateNumericProof(report));
});


test("decoded receiver ring gap rejects before accepting retained suffix", () => {
  const aggregate = new PacketAggregate();
  const receiver = {mark:() => 3,packetsAfter:() => [packet(2),packet(3)]};
  assert.throws(() => drainPacketRing(receiver,0,aggregate,() => 1),/ring lost/);
  assert.equal(aggregate.count,0);
  assert.equal(drainPacketRing(receiver,1,aggregate,() => 1),3);
  assert.equal(aggregate.count,2);
});
test("one-packet universe and disabled sequence cannot falsely pass full-minute proof", () => {
  for (const mutate of [
    row => { row.proof.packets.rows[0].packets=1;row.proof.packets.packet_count-=2399; },
    row => { row.proof.packets.rows[0].zero_sequence_packets=2400; },
    row => { row.proof.packets.rows[0].packets=2397;row.proof.packets.packet_count-=3; },
    row => { row.proof.packets.rows[0].packets=2409;row.proof.packets.packet_count+=9; },
  ]) { const measured=result();mutate(measured);assert.equal(numericProof(measured,completedProgress()).all_gates_passed,0); }
  for(const count of [2398,2408]) { const measured=result();measured.proof.packets.rows[0].packets=count;measured.proof.packets.packet_count+=count-2400;assert.equal(numericProof(measured,completedProgress()).all_gates_passed,1); }
});


test("schema3 Stage gaps preserve per-lane count, min/max offsets and edge diagnostics", () => {
  const raw=stage(); raw.nativeStage.activity=raw.nativeStage.activity.filter(row => !row.followPreload);
  const diagnostic=summarizeStageWindow(raw,100_000,160_000);
  assert.equal(diagnostic.lanes[0].validation_passed,1);
  assert.deepEqual([diagnostic.lanes[0].retained_samples,diagnostic.lanes[0].first_sample_offset_ms,diagnostic.lanes[0].last_sample_offset_ms],[63,-1000,61000]);
  assert.equal(diagnostic.lanes[1].available,0);assert.equal(diagnostic.lanes[1].max_gap_ms,60000);
  const measured=result();measured.proof.stage=diagnostic;
  const report=numericProof(measured,{complete:0,current_stage_code:13,failure_stage_code:13});
  assert.equal(report.stage_report_available,1);assert.equal(report.stage_validation_passed,0);
  assert.equal(report.measurement_available,1);assert.equal(report.completed_frames,2400);assert.equal(report.resources_available,1);assert.equal(report.udp_passed,1);assert.equal(report.all_gates_passed,0);
});
test("atomic checkpoint state retains completed metrics on Stage wait failure and first failure through cleanup", async () => {
  const writes=[];const proof=new ProofCheckpoint("private-test-no-file",{write:async (_file,row) => {writes.push(structuredClone(row));}});
  await proof.enter(7);
  const measured=result();delete measured.proof.stage;delete measured.proof.interaction;
  Object.assign(proof.result,measured);
  await proof.enter(12);await proof.fail();await proof.enter(14);await proof.fail(14);await proof.finish(1);
  const last=writes.at(-1);assert.equal(last.schema_version,3);assert.equal(last.failure_stage_code,12);assert.equal(last.current_stage_code,14);assert.equal(last.complete,0);assert.equal(last.cleanup_complete,1);
  assert.equal(last.completed_frames,2400);assert.equal(last.measurement_available,1);assert.equal(last.udp_available,1);assert.equal(last.resources_available,1);assert.equal(last.stage_report_available,0);assert.equal(last.all_gates_passed,0);
  assert(writes.every(row => row.all_gates_passed === 0));
});
test("trusted phase and availability mutation cannot manufacture a successful report", async () => {
  const complete=numericProof(result(),completedProgress());assert.equal(complete.all_gates_passed,1);
  for(const mutate of [row => {row.failure_stage_code="timeout private-path";},row => {row.failure_stage_code=16;},row => {row.current_stage_code=7.5;},row => {row.measurement_available=0;},row => {row.resources_available=0;},row => {row.stage_validation_passed=0;},row => {row.cleanup_complete=0;},row => {row.stage_lanes[0].retained_samples++;}]) {
    const bad=structuredClone(complete);mutate(bad);assert.throws(() => validateNumericProof(bad));
  }
  const proof=new ProofCheckpoint("no-file",{write:async () => {}});
  assert.throws(() => proof.enter("Stage timeout"));assert.throws(() => proof.fail(99));
});
test("checkpoint writes are ordered and an unsuccessful write cannot erase the last durable phase", async () => {
  const writes=[];let fail=false;
  const proof=new ProofCheckpoint("no-file",{write:async (_file,row) => {if(fail)throw new Error("fake write failure");writes.push(structuredClone(row));}});
  await proof.enter(7);fail=true;await assert.rejects(proof.enter(8),/fake write/);assert.equal(writes.at(-1).current_stage_code,7);
  fail=false;await proof.fail(8);await proof.enter(14);await proof.finish(0);
  assert.equal(writes.at(-1).failure_stage_code,8);assert.equal(writes.at(-1).cleanup_complete,0);assert.equal(writes.at(-1).all_gates_passed,0);
});
test("successful checkpoint requires every completed phase, not zero-filled unavailable metrics", async () => {
  const proof=new ProofCheckpoint("no-file",{write:async () => {}});
  await assert.rejects(async () => {await proof.finish(1);});
  Object.assign(proof.result,result());proof.progress.sheet_liveness_checked=1;proof.progress.sheet_liveness_passed=1;
  const report=await proof.finish(1);assert.equal(report.all_gates_passed,1);assert.equal(report.current_stage_code,0);
});


test("result objects never imply unobserved Sheet liveness or cleanup", () => {
  const report=numericProof(result());
  assert.equal(report.complete,0);assert.equal(report.sheet_liveness_checked,0);assert.equal(report.sheet_liveness_passed,0);assert.equal(report.cleanup_complete,0);assert.equal(report.all_gates_passed,0);
  assert.equal(report.completed_frames,2400);assert.equal(report.measurement_available,1);
});
test("real atomic checkpoint replacement retains schema3 and leaves no temporary siblings", async () => {
  const directory=await mkdtemp(path.join(artifactPaths.tmp,"tl775-proof-schema3-test-"));
  try {
    const file=path.join(directory,"numeric.json"),proof=new ProofCheckpoint(file);
    await proof.enter(7);await Promise.all([proof.enter(8),proof.enter(9)]);
    const report=validateNumericProof(JSON.parse(await readFile(file,"utf8")));
    assert.equal(report.current_stage_code,9);assert.equal(report.complete,0);assert.equal(report.all_gates_passed,0);
    assert.equal((await stat(file)).mode & 0o777,0o600);assert.deepEqual(await readdir(directory),["numeric.json"]);
  } finally {await rm(directory,{recursive:true});}
});


test("unfiltered preload presentations retain activity despite legacy deduplication while live source advances", () => {
  const raw=stage();for(const frame of raw.nativeStage.activity)if(frame.followPreload)frame.sourceFrame=7;
  raw.nativeStage.frames=raw.nativeStage.frames.filter(row => !row.followPreload || row.sequence === 1);
  const summary=summarizeStageWindow(raw,100_000,160_000);
  assert.equal(summary.validation_passed,1);
  for(const lane of summary.lanes) {assert.equal(lane.presentation_continuity_passed,1);assert.equal(lane.idle_continuity_passed,0);assert.equal(lane.observed_samples,61);}
  assert.equal(summary.lanes[1].filtered_samples,1);assert.equal(summary.lanes[0].source_continuity_passed,1);
});
test("static preload may intentionally idle only behind fresh same-attachment healthy unchanged main-loop evidence", () => {
  const summary=summarizeStageWindow(staticPreloadStage(),100_000,160_000);
  assert.equal(summary.validation_passed,1);
  assert.equal(summary.lanes[0].presentation_continuity_passed,1);
  const lane=summary.lanes[1];assert.equal(lane.presentation_continuity_passed,0);assert.equal(lane.idle_continuity_passed,1);assert.equal(lane.observed_samples,0);assert.equal(lane.attachment_count,1);assert.equal(lane.heartbeat_link_failures,0);assert(lane.heartbeat_observed_samples > 100);
  const measured=result();measured.proof.stage=summary;assert.equal(numericProof(measured,completedProgress()).all_gates_passed,1);
});
test("static idle rejects loop-alive-only, draw errors, hidden/detached panes, wrong prior output and stale receive", () => {
  for(const mutate of [
    hb => {hb.idleUnchanged=false;},hb => {hb.drawHealthy=false;},hb => {hb.attached=false;},hb => {hb.visible=false;},
    hb => {hb.sourceFrame++;},hb => {hb.width++;},hb => {hb.lastPresentedSequence++;},hb => {hb.epochMicros-=6_000_000;},
  ]) {
    const raw=staticPreloadStage();raw.nativeStage.heartbeats.forEach(mutate);
    const summary=summarizeStageWindow(raw,100_000,160_000);assert.equal(summary.validation_passed,0);assert.equal(summary.lanes[1].idle_continuity_passed,0);
  }
});
test("foreign/reused attachments, reordered heartbeat and missing main-loop window fail closed", () => {
  for(const mutate of [
    raw => {raw.nativeStage.heartbeats[20].attachmentId="foreign";},
    raw => {raw.nativeStage.heartbeats[20].paneId="main:foreign-pane";},
    raw => {raw.nativeStage.heartbeats[20].sequence=1;},
    raw => {raw.nativeStage.heartbeats[20].receivedEpochMicros=raw.nativeStage.heartbeats[19].receivedEpochMicros-1;},
    raw => {raw.nativeStage.heartbeats=raw.nativeStage.heartbeats.filter((_,i) => i < 20 || i > 40);},
    raw => {raw.nativeStage.heartbeats.forEach(hb => {hb.attachmentId="new-helper-reusing-pane";});},
    raw => {raw.nativeStage.observerOverflow.heartbeats=1;},
  ]) {const raw=staticPreloadStage();mutate(raw);assert.equal(summarizeStageWindow(raw,100_000,160_000).validation_passed,0);}
});
test("live pane never substitutes idle heartbeat for actual presentation continuity", () => {
  const raw=staticPreloadStage();
  raw.nativeStage.activity=raw.nativeStage.activity.map(frame => ({...frame,followPreload:!frame.followPreload}));
  raw.nativeStage.heartbeats=raw.nativeStage.heartbeats.map(hb => ({...hb,followPreload:false}));
  const summary=summarizeStageWindow(raw,100_000,160_000);
  assert.equal(summary.lanes[0].idle_continuity_passed,0);assert.equal(summary.validation_passed,0);
});
test("public idle evidence is closed numeric, never pane/attachment strings or fabricated live idle success", () => {
  const measured=result();measured.proof.stage=summarizeStageWindow(staticPreloadStage(),100_000,160_000);
  const report=numericProof(measured,completedProgress());
  assert(!JSON.stringify(report).includes("preload-private-id"));
  for(const mutate of [row => {row.stage_lanes[1].attachmentId="private";},row => {row.stage_lanes[0].idle_continuity_passed=1;},row => {row.stage_lanes[1].heartbeat_link_failures=1;},row => {row.stage_lanes[1].observer_failed=1;}]) {const bad=structuredClone(report);mutate(bad);assert.throws(() => validateNumericProof(bad));}
});


test("fresh draw timestamps cannot hide a stalled live lighting source", () => {
  const raw=stage();for(const frame of raw.nativeStage.activity)if(!frame.followPreload)frame.sourceFrame=7;
  const summary=summarizeStageWindow(raw,100_000,160_000);
  assert.equal(summary.lanes[0].observed_samples,61);assert.equal(summary.lanes[0].max_gap_ms,1000);
  assert.equal(summary.lanes[0].source_change_samples,0);assert.equal(summary.lanes[0].source_continuity_passed,0);assert.equal(summary.lanes[0].presentation_continuity_passed,0);assert.equal(summary.validation_passed,0);
});

test("actual TS observer retains pre-filter activity and separates attachment generations", async () => {
  const source=await readFile(new URL("../apps/light-desktop/src/NativePackagedStageBenchmarkApp.tsx",import.meta.url),"utf8");
  const begin=source.indexOf("interface NativeFrameSample"),end=source.indexOf("export function NativePackagedStageBenchmarkApp");
  const {transform}=await import("esbuild");
  const compiled=await transform(source.slice(begin,end),{loader:"ts",format:"cjs",target:"node22"});
  const {retainNativeObservations,appendNativeFrames}=new Function(`${compiled.code};return {retainNativeObservations,appendNativeFrames};`)();
  const frame={...stage().nativeStage.activity[0],quality:"high",sourceInputEpochMicros:0};
  const raw=[],filtered=[],heartbeats=[],overflow={activity:0,heartbeats:0,errors:0},latest=new Map();
  for(const sequence of [1,2]) {
    const batch={frames:[{...frame,sequence}],heartbeats:[]};
    retainNativeObservations(batch,raw,heartbeats,overflow);appendNativeFrames(filtered,batch.frames,latest);
  }
  assert.equal(raw.length,2);assert.equal(filtered.length,1);
  retainNativeObservations({frames:[frame,{...frame,attachmentId:"replacement"}],heartbeats:[]},raw,heartbeats,overflow);
  assert.equal(raw.length,4);assert.notEqual(raw[2].attachmentId,raw[3].attachmentId);
  const cappedActivity=Array(20_000).fill(frame),cappedHeartbeats=Array(10_000).fill({});
  retainNativeObservations({frames:[frame],heartbeats:[{}]},cappedActivity,cappedHeartbeats,overflow);
  assert.equal(cappedActivity.length,20_000);assert.equal(cappedHeartbeats.length,10_000);assert.deepEqual(overflow,{activity:1,heartbeats:1,errors:0});
});

test("known warmup and terminal presentation errors retain diagnostics without poisoning a valid measured window", () => {
  const raw=stage(), template=raw.nativeStage.activity.find(row => row.followPreload);
  raw.nativeStage.activity.unshift({...template,sequence:0,sourceFrame:0,presentedEpochMicros:90_000_000,drawCalls:0});
  raw.nativeStage.activity.push({...template,sequence:100,sourceFrame:100,presentedEpochMicros:170_000_000,width:0});
  const summary=summarizeStageWindow(raw,100_000,160_000),lane=summary.lanes[1];
  assert.equal(summary.validation_passed,1);assert.equal(lane.presentation_continuity_passed,1);
  assert.equal(lane.invalid_samples,2);assert.equal(lane.activity_invalid_before_samples,1);assert.equal(lane.activity_invalid_after_samples,1);
  assert.equal(lane.activity_invalid_in_window_samples,0);assert.equal(lane.activity_invalid_unlocalizable_samples,0);
  assert.equal(lane.activity_invalid_content_samples,1);assert.equal(lane.activity_invalid_dimensions_samples,1);
  const measured=result();measured.proof.stage=summary;
  assert.equal(numericProof(measured,completedProgress()).all_gates_passed,1);
});

test("presentation errors inside the closed measured window remain strict gate failures", () => {
  for(const millis of [100_000,130_000,160_000]) {
    const raw=stage(),template=raw.nativeStage.activity.find(row => row.followPreload);
    raw.nativeStage.activity.push({...template,presentedEpochMicros:millis*1000,drawCalls:0});
    const summary=summarizeStageWindow(raw,100_000,160_000),lane=summary.lanes[1];
    assert.equal(lane.invalid_samples,1);assert.equal(lane.activity_invalid_in_window_samples,1);
    assert.equal(lane.activity_invalid_content_samples,1);assert.equal(lane.presentation_continuity_passed,0);assert.equal(summary.validation_passed,0);
  }
});

test("missing owner or untrusted clocks cannot disguise invalid presentation evidence as warmup", () => {
  for(const patch of [{paneId:"foreign-owner"},{presentedEpochMicros:NaN},{presentedEpochMicros:900_000_000}]) {
    const raw=stage(),template=raw.nativeStage.activity.find(row => row.followPreload);
    raw.nativeStage.activity.unshift({...template,presentedEpochMicros:90_000_000,drawCalls:0,...patch});
    const summary=summarizeStageWindow(raw,100_000,160_000),lane=summary.lanes[1];
    assert.equal(lane.activity_invalid_unlocalizable_samples,1);assert.equal(lane.activity_invalid_before_samples,0);assert.equal(summary.validation_passed,0);
  }
});

test("an invalid out-of-window presentation cannot supply a missing continuity anchor", () => {
  const raw=stage(),template=raw.nativeStage.activity.find(row => row.followPreload);
  raw.nativeStage.activity=raw.nativeStage.activity.filter(row => !row.followPreload || row.presentedEpochMicros > 100_000_000);
  raw.nativeStage.activity.unshift({...template,presentedEpochMicros:99_000_000,drawCalls:0});
  const summary=summarizeStageWindow(raw,100_000,160_000),lane=summary.lanes[1];
  assert.equal(lane.activity_invalid_before_samples,1);assert.equal(lane.observed_samples,60);assert.equal(lane.bracketed,0);assert.equal(summary.validation_passed,0);
});

test("heartbeat invalid scope is conserved and measured or unlocalizable errors still block static idle", () => {
  for(const [millis,expectedScope,pass] of [[90_000,"before",1],[170_000,"after",1],[130_000,"in_window",0]]) {
    const raw=staticPreloadStage(),template=raw.nativeStage.heartbeats[0];
    raw.nativeStage.heartbeats.push({...template,epochMicros:millis*1000,receivedEpochMicros:millis*1000,sequence:-1});
    const summary=summarizeStageWindow(raw,100_000,160_000),lane=summary.lanes[1];
    assert.equal(lane.heartbeat_invalid_samples,1);assert.equal(lane[`heartbeat_invalid_${expectedScope}_samples`],1);
    assert.equal(lane.heartbeat_invalid_sequence_source_samples,1);assert.equal(lane.idle_continuity_passed,pass);assert.equal(summary.validation_passed,pass);
  }
  const raw=staticPreloadStage();raw.nativeStage.heartbeats.push({...raw.nativeStage.heartbeats[0],epochMicros:NaN,receivedEpochMicros:90_000_000});
  const summary=summarizeStageWindow(raw,100_000,160_000);
  assert.equal(summary.lanes[1].heartbeat_invalid_unlocalizable_samples,1);assert.equal(summary.validation_passed,0);
});

test("one measured heartbeat link failure still blocks idle fallback independently of scoped errors", () => {
  const raw=staticPreloadStage();raw.nativeStage.heartbeats[20].lastPresentedSequence=99;
  const summary=summarizeStageWindow(raw,100_000,160_000),lane=summary.lanes[1];
  assert.equal(lane.heartbeat_invalid_samples,0);assert.equal(lane.heartbeat_link_failures,1);assert.equal(lane.idle_continuity_passed,0);assert.equal(summary.validation_passed,0);
});

test("public invalid diagnostics require scope conservation and cannot silently reinterpret an older report", () => {
  const complete=numericProof(result(),completedProgress());
  for(const mutate of [row => {row.stage_lanes[1].activity_invalid_before_samples=1;},row => {row.stage_lanes[1].heartbeat_invalid_clock_samples=1;},row => {delete row.stage_lanes[1].activity_invalid_unlocalizable_samples;}]) {
    const bad=structuredClone(complete);mutate(bad);assert.throws(() => validateNumericProof(bad));
  }
});

test("null or undefined raw observations retain numeric failure without assigning a fictitious lane", () => {
  for(const kind of ["activity","heartbeats"])for(const value of [null,undefined]) {
    const raw=stage();raw.nativeStage[kind].push(value);
    const summary=summarizeStageWindow(raw,100_000,160_000);
    assert.equal(summary.report_available,1);assert.equal(summary.validation_passed,0);
    assert(summary.lanes.every(row => row.observer_failed === 1));
    // Missing lane identity is a report-wide observer failure, never invented ownership.
    assert(summary.lanes.every(row => row.activity_invalid_unlocalizable_samples === 0 && row.heartbeat_invalid_unlocalizable_samples === 0));
    const measured=result();measured.proof.stage=summary;
    const numeric=numericProof(measured,completedProgress());
    assert.equal(numeric.schema_version,3);assert.equal(numeric.all_gates_passed,0);
    assert.deepEqual(validateNumericProof(JSON.parse(JSON.stringify(numeric))),numeric);
  }
});

test("missing fields on an otherwise assigned lane retain bounded unlocalizable reason diagnostics", () => {
  for(const [kind,prefix] of [["activity","activity"],["heartbeats","heartbeat"]]) {
    const raw=stage();raw.nativeStage[kind].push({followPreload:true});
    const summary=summarizeStageWindow(raw,100_000,160_000),lane=summary.lanes[1];
    assert.equal(summary.validation_passed,0);assert.equal(lane[`${prefix}_invalid_unlocalizable_samples`],1);
    for(const reason of ["identity","clock","sequence_source","dimensions","content"])assert.equal(lane[`${prefix}_invalid_${reason}_samples`],1);
    const measured=result();measured.proof.stage=summary;
    assert.equal(numericProof(measured,completedProgress()).all_gates_passed,0);
  }
});

test("numeric schema3 explicitly rejects schema2 while native source complete records remain schemaVersion2", () => {
  const source=stage();assert.equal(source.schemaVersion,2);
  assert.equal(summarizeStageWindow(source,100_000,160_000).validation_passed,1);
  const numeric=numericProof(result(),completedProgress());assert.equal(numeric.schema_version,3);
  numeric.schema_version=2;assert.throws(() => validateNumericProof(numeric));
});


async function readinessClass() {
  const source=await readFile(new URL("../apps/light-desktop/src/nativePackagedStageReadiness.ts",import.meta.url),"utf8");
  const {transform}=await import("esbuild");
  const compiled=await transform(source,{loader:"ts",format:"cjs",target:"node22"});
  const module={exports:{}};new Function("module",compiled.code)(module);
  return module.exports.NativeStageReadiness;
}
function readyInputs() {
  const live={...stage().nativeStage.activity[0],sequence:10,sourceFrame:100,presentedEpochMicros:117_100_000};
  const next={...live,sequence:11,sourceFrame:101,presentedEpochMicros:117_650_000};
  const preload={...stage().nativeStage.activity.find(row => row.followPreload),sequence:2,sourceFrame:7,presentedEpochMicros:104_000_000};
  const heartbeat=(frame,idle=false) => ({paneId:frame.paneId,attachmentId:frame.attachmentId,sequence:20,epochMicros:117_700_000,receivedEpochMicros:117_710_000,lastPresentedSequence:frame.sequence,sourceFrame:frame.sourceFrame,followPreload:frame.followPreload,width:frame.width,height:frame.height,attached:true,visible:true,idleUnchanged:idle,drawHealthy:true});
  return {live,next,preload,heartbeats:[heartbeat(next),heartbeat(preload,true)]};
}

test("actual TS readiness waits for post-lifecycle advancing Live evidence instead of starting at a 17-second timer", async () => {
  const Ready=await readinessClass(),ready=new Ready(),v=readyInputs();
  const oldLive={...v.live,sequence:1,sourceFrame:1,presentedEpochMicros:110_000_000};
  assert.equal(ready.observe({frames:[oldLive,v.preload],heartbeats:[]},100_000,116_999,false),null);
  // The old fixed-delay runner would begin at117000; this surface has no post-lifecycle Live picture yet.
  assert.equal(ready.observe({frames:[],heartbeats:[]},100_000,117_000,false),null);
  assert.equal(ready.observe({frames:[v.live],heartbeats:[]},100_000,117_750,false),null);
  const record=ready.observe({frames:[v.next],heartbeats:v.heartbeats},100_000,117_800,false);
  assert(record);assert.equal(record.eligibleEpochMillis,117_000);assert.equal(record.lanes[0].previousLiveFrame.sourceFrame,100);
  assert.deepEqual(validateStageReady(record),record);
  assert.equal(ready.observe({frames:[v.next],heartbeats:v.heartbeats},100_000,117_900,false),null,"private readiness is one shot");
});

test("readiness rejects stale, foreign, hidden, detached, unhealthy and nonadvancing evidence", async () => {
  const Ready=await readinessClass();
  for(const mutate of [
    v=>{v.heartbeats[0].attachmentId="old-attachment";},v=>{v.heartbeats[0].attached=false;},
    v=>{v.heartbeats[0].visible=false;},v=>{v.heartbeats[0].drawHealthy=false;},
    v=>{v.heartbeats[0].width++;},v=>{v.heartbeats[0].sourceFrame++;},
    v=>{v.heartbeats[0].epochMicros=110_000_000;},v=>{v.heartbeats[0].receivedEpochMicros=120_000_000;},
    v=>{v.next.sourceFrame=v.live.sourceFrame;v.heartbeats[0].sourceFrame=v.live.sourceFrame;},
    v=>{v.heartbeats[1].idleUnchanged=false;},v=>{v.heartbeats[1].drawHealthy=false;},
    v=>{v.preload.paneId=v.live.paneId;v.preload.attachmentId=v.live.attachmentId;v.heartbeats[1].paneId=v.live.paneId;v.heartbeats[1].attachmentId=v.live.attachmentId;},
  ]) {
    const v=readyInputs();mutate(v);const ready=new Ready();
    assert.equal(ready.observe({frames:[v.live,v.next,v.preload],heartbeats:v.heartbeats},100_000,117_800,false),null);
  }
  const v=readyInputs();assert.equal(new Ready().observe({frames:[v.live,v.next,v.preload],heartbeats:v.heartbeats},100_000,117_800,true),null);
});

test("readiness accepts fresh presented Preload and fails closed on bounded raw witness overflow", async () => {
  const Ready=await readinessClass(),v=readyInputs();
  v.preload.presentedEpochMicros=117_600_000;v.heartbeats[1].idleUnchanged=false;
  const record=new Ready().observe({frames:[v.live,v.next,v.preload],heartbeats:v.heartbeats},100_000,117_800,false);
  assert(record);validateStageReady(record);
  const many=Array.from({length:513},(_,i)=>({...v.live,sequence:i+1,sourceFrame:i,presentedEpochMicros:117_000_000+i*1000}));
  const last=many.at(-1);v.heartbeats[0].lastPresentedSequence=last.sequence;v.heartbeats[0].sourceFrame=last.sourceFrame;
  assert.equal(new Ready().observe({frames:many.concat([v.preload]),heartbeats:v.heartbeats},100_000,117_800,false),null);
});

test("private readiness validation rejects changed evidence and waits for an explicit record", async () => {
  const Ready=await readinessClass(),v=readyInputs(),record=new Ready().observe({frames:[v.live,v.next,v.preload],heartbeats:v.heartbeats},100_000,117_800,false);
  for(const mutate of [r=>{r.eligibleEpochMillis--;},r=>{r.lanes[0].heartbeat.attached=false;},r=>{r.lanes[0].frame.presentedEpochMicros=118_000_000;},r=>{r.lanes[0].previousLiveFrame.sourceFrame=r.lanes[0].frame.sourceFrame;},r=>{r.lanes[1].heartbeat.idleUnchanged=false;}]) {
    const bad=structuredClone(record);mutate(bad);assert.throws(()=>validateStageReady(bad));
  }
  const directory=await mkdtemp(path.join(artifactPaths.tmp,"tl775-ready-test-"));
  try {
    const {writeFile}=await import("node:fs/promises"),file=path.join(directory,"ready.jsonl");
    await writeFile(file,JSON.stringify(record)+"\n");
    assert.deepEqual(await waitStageReady(file,()=>{},1),record);
    await writeFile(file,JSON.stringify({schemaVersion:2,kind:"stage-started"})+"\n");
    await assert.rejects(()=>waitStageReady(file,()=>{},1),/Stage stage-ready timed out/);
  } finally {await rm(directory,{recursive:true});}
});


test("readiness heartbeat monotonic history cannot be hidden by last-write-wins or later progress", async () => {
  const Ready=await readinessClass();
  for(const mutate of [
    hb=>{hb.sequence=19;},hb=>{hb.sequence=0;},hb=>{hb.sourceFrame=100;},hb=>{hb.sourceFrame=0;},
    hb=>{hb.lastPresentedSequence=10;},hb=>{hb.epochMicros=117_699_000;},
    hb=>{hb.receivedEpochMicros=117_709_000;},hb=>{hb.epochMicros=0;hb.receivedEpochMicros=0;},
  ]) {
    const v=readyInputs(),ready=new Ready();
    assert.equal(ready.observe({frames:[v.live,v.next],heartbeats:[v.heartbeats[0]]},100_000,117_800,false),null);
    const bad={...v.heartbeats[0],sequence:21,epochMicros:117_720_000,receivedEpochMicros:117_730_000};mutate(bad);
    assert.equal(ready.observe({frames:[],heartbeats:[bad]},100_000,117_850,false),null);
    const recovered={...v.heartbeats[0],sequence:22,epochMicros:117_740_000,receivedEpochMicros:117_750_000};
    assert.equal(ready.observe({frames:[v.preload],heartbeats:[recovered,v.heartbeats[1]]},100_000,117_900,false),null,"normal progress cannot erase rejected history");
  }
});

test("readiness rejects same-attachment frame sequence source and time regressions including positive-to-zero", async () => {
  const Ready=await readinessClass();
  for(const mutate of [
    frame=>{frame.sequence=9;},frame=>{frame.sequence=0;},frame=>{frame.sourceFrame=99;},frame=>{frame.sourceFrame=0;},
    frame=>{frame.presentedEpochMicros=117_000_000;},frame=>{frame.presentedEpochMicros=0;},
  ]) {
    const v=readyInputs(),ready=new Ready();
    assert.equal(ready.observe({frames:[v.live],heartbeats:[]},100_000,117_200,false),null);
    const bad={...v.next,sequence:11,sourceFrame:101,presentedEpochMicros:117_300_000};mutate(bad);
    assert.equal(ready.observe({frames:[bad],heartbeats:[]},100_000,117_400,false),null);
    const recovered={...v.next,sequence:12,sourceFrame:102};v.heartbeats[0].lastPresentedSequence=12;v.heartbeats[0].sourceFrame=102;
    assert.equal(ready.observe({frames:[recovered,v.preload],heartbeats:v.heartbeats},100_000,117_800,false),null);
  }
});

test("malformed or unknown-lane post-lifecycle readiness evidence is sticky rather than silently skipped", async () => {
  const Ready=await readinessClass();
  for(const kind of ["frames","heartbeats"])for(const bad of [null,undefined,{followPreload:"unknown"},
    kind === "frames" ? {...readyInputs().next,drawCalls:-1} : {...readyInputs().heartbeats[0],sequence:NaN},
    kind === "frames" ? {...readyInputs().next,presentedEpochMicros:undefined} : {...readyInputs().heartbeats[0],receivedEpochMicros:undefined}]) {
    const v=readyInputs(),ready=new Ready(),batch={frames:[],heartbeats:[]};batch[kind].push(bad);
    assert.equal(ready.observe(batch,100_000,117_800,false),null);
    assert.equal(ready.observe({frames:[v.live,v.next,v.preload],heartbeats:v.heartbeats},100_000,117_900,false),null);
  }
});

test("well-formed post-lifecycle initial blanks delay readiness and cannot manufacture the two painted witnesses", async () => {
  const Ready=await readinessClass();
  for(const zeros of [false,true]) {
    const v=readyInputs(),ready=new Ready();
    const blank={...v.live,sequence:zeros?0:9,sourceFrame:zeros?0:99,presentedEpochMicros:117_020_000,width:zeros?0:640,height:zeros?0:480,drawCalls:0};
    const hb={...v.heartbeats[0],sequence:zeros?0:19,lastPresentedSequence:blank.sequence,sourceFrame:blank.sourceFrame,width:blank.width,height:blank.height,epochMicros:117_030_000,receivedEpochMicros:117_040_000,attached:false,visible:false,drawHealthy:false};
    assert.equal(ready.observe({frames:[blank],heartbeats:[hb]},100_000,117_050,false),null);
    if(zeros) assert.equal(ready.observe({frames:[{...blank,presentedEpochMicros:117_021_000}],heartbeats:[{...hb,epochMicros:117_031_000,receivedEpochMicros:117_041_000}]},100_000,117_060,false),null,"zero-to-zero startup state remains unready without rejecting recovery");
    assert.equal(ready.observe({frames:[v.live],heartbeats:[]},100_000,117_200,false),null,"blank is not the earlier painted Live witness");
    const record=ready.observe({frames:[v.next,v.preload],heartbeats:v.heartbeats},100_000,117_800,false);
    assert(record);assert.equal(record.lanes[0].previousLiveFrame.sequence,v.live.sequence);validateStageReady(record);
  }
});

test("known pre-lifecycle warmup invalid evidence does not reject later fully qualified readiness", async () => {
  const Ready=await readinessClass(),v=readyInputs(),ready=new Ready();
  assert.equal(ready.observe({frames:[{...v.live,presentedEpochMicros:110_000_000,drawCalls:-1}],heartbeats:[]},100_000,117_050,false),null);
  const record=ready.observe({frames:[v.live,v.next,v.preload],heartbeats:v.heartbeats},100_000,117_800,false);
  assert(record);validateStageReady(record);
});
