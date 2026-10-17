import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { PacketAggregate, drainPacketRing, summarizeRamp, summarizeStageWindow, numericProof, validateNumericProof, verifyWorkloadFunctions } from "./desktop-performance-576-proof-observers.mjs";

function packet(sequence = 1, universe = 101) {
  return {protocol:"artnet",universe,sequence,slots:new Uint8Array(512),artnet:{payloadLength:512,protocolVersion:14}};
}
function stage() {
  return {kind:"complete",measurementSurface:"packaged-tauri-native-stage",nativeStage:{frames:[false,true].flatMap(followPreload => Array.from({length:63},(_,i) => ({paneId:followPreload?"preload-private-id":"live-private-id",sequence:i,sourceFrame:i,lane:followPreload?"preload":"normal",followPreload,width:640,height:480,drawCalls:1,presentedEpochMicros:(99_000+i*1000)*1000})))}};
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
test("Stage configuration labels alone, missing lane, edge gap or mismatched provenance fail", () => {
  assert.throws(() => summarizeStageWindow({kind:"complete",measurementSurface:"packaged-tauri-native-stage",activeUiSurfaces:["stage-native-live","stage-native-preload"]},100_000,160_000),/cap/);
  for(const mutate of [
    report => {report.nativeStage.frames=report.nativeStage.frames.filter(row => row.lane === "normal");},
    report => {report.nativeStage.frames=report.nativeStage.frames.filter(row => !(row.lane === "normal" && row.sequence > 10 && row.sequence < 20));},
    report => {report.nativeStage.frames[0].followPreload=true;},
    report => {report.nativeStage.frames=report.nativeStage.frames.filter(row => row.sequence >= 2);},
  ]) { const report=stage();mutate(report);assert.throws(() => summarizeStageWindow(report,100_000,160_000)); }
});
test("public report roundtrip is closed, numeric, workload exact and free of private metadata", () => {
  const report = numericProof(result()); assert.equal(report.all_gates_passed,1);
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
    const report=numericProof(measured);assert.equal(report.complete,1);assert.equal(report.all_gates_passed,0);assert.equal(report.completed_frames,2400);
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
  ]) { const measured=result();mutate(measured);assert.equal(numericProof(measured).all_gates_passed,0); }
  for(const count of [2398,2408]) { const measured=result();measured.proof.packets.rows[0].packets=count;measured.proof.packets.packet_count+=count-2400;assert.equal(numericProof(measured).all_gates_passed,1); }
});
