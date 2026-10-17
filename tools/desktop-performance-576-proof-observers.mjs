// Observers for the focused released Desk proof. Raw records never leave private scratch.
import assert from "node:assert/strict";
import crypto from "node:crypto";
import { readFile, stat, writeFile, mkdir } from "node:fs/promises";
import path from "node:path";
import { createRequire } from "node:module";

const sha = value => crypto.createHash("sha256").update(value).digest("hex");
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
const ROOT = path.resolve(import.meta.dirname, "..");
const FUNCTIONS = {
  prepareMixed: "8865a2710c533777f195f124662f2427ebfa98df9051a3a05c00187bf65ed57e",
  installDynamics: "adc9f2a0846a6608d83c4f25fdcd00202d657b00341f3f6cb0d64c8eae94fbba",
  summarizePatch: "ba2e0f71e48ebc14ca7233d41846ea0b45bbc524c3377c84a56aff633b0ed615",
};
export async function verifyWorkloadFunctions(functions) {
  assert.deepEqual(Object.keys(functions).sort(), Object.keys(FUNCTIONS).sort());
  for (const [name, fn] of Object.entries(functions)) assert.equal(sha(fn.toString()), FUNCTIONS[name], `executed workload drift: ${name}`);
  // Imports remain the exact accepted workload constructors; workflow binds package bytes too.
  for (const [file, digest] of Object.entries({
    "stage-large-scene.mjs": "b5306a234850586cb7a3c71d296b042477166ddc7dbae2f670b2658443ca7419",
    "stage-dynamics-scene.mjs": "089ef51628cd9e564999f3472eda9b781217ac95d9071a63183268a2849e3725",
    "../tests/bench/core/protocols.ts": "face0cd976855cc8aa28432b6d21dc022fbc68668d94b7b66f28199fc3c8b1de",
  })) assert.equal(sha(await readFile(path.resolve(import.meta.dirname, file))), digest, `dependency drift: ${file}`);
}
function uint(value) { assert(Number.isSafeInteger(value) && value >= 0, "invalid unsigned number"); return value; }
function nonnegative(value) { assert(typeof value === "number" && Number.isFinite(value) && value >= 0, "invalid finite number"); return value; }

export class PacketAggregate {
  constructor() {
    this.count = 0; this.rows = Array.from({length:16}, () => ({packets:0, sequence_discontinuities:0, zero_sequence_packets:0}));
    this.lastSequence = Array(16).fill(null); this.staticRows = []; this.error = null;
  }
  add(packet, monotonicMillis) {
    assert(this.count < 65_536, "packet count cap");
    assert(packet.protocol === "artnet" && Number.isSafeInteger(packet.universe) && packet.universe >= 101 && packet.universe <= 116, "unexpected protocol/universe");
    assert(packet.slots.length === 512 && packet.artnet?.payloadLength === 512 && packet.artnet?.protocolVersion >= 14, "invalid full DMX payload");
    const sequence = uint(packet.sequence); assert(sequence <= 255, "invalid sequence");
    const index = packet.universe-101, row = this.rows[index], previous = this.lastSequence[index];
    // Art-Net sequence zero explicitly disables ordering. Do not pretend it proves continuity.
    if (sequence !== 0 && previous !== null && previous !== 0 && sequence !== (previous === 255 ? 1 : previous+1)) row.sequence_discontinuities++;
    this.lastSequence[index] = sequence; row.packets++; if (sequence === 0) row.zero_sequence_packets++; this.count++;
    if (index === 0) {
      assert(this.staticRows.length < 4096, "static fixture sample cap");
      this.staticRows.push({ms:nonnegative(monotonicMillis), value:uint(packet.slots[510])});
    }
  }
  snapshot() {
    if (this.error) throw this.error;
    return {packet_count:this.count, rows:this.rows.map(row => ({...row}))};
  }
}

export function drainPacketRing(receiver, cursor, aggregate, clock = () => performance.now()) {
  const next = receiver.mark(), rows = receiver.packetsAfter(cursor);
  assert.equal(rows.length, next-cursor, "decoded receiver ring lost packets");
  for (const packet of rows) aggregate.add(packet, clock());
  return next;
}

export function summarizeRamp(rows, start, from, to) {
  nonnegative(start); uint(from); uint(to); assert(from !== to);
  assert(Array.isArray(rows) && rows.length <= 4096);
  let previousTime = -1;
  for (const row of rows) { nonnegative(row.ms); uint(row.value); assert(row.value <= 255 && row.ms >= previousTime, "invalid ramp sample"); previousTime = row.ms; }
  const before = rows.filter(row => row.ms <= start).at(-1);
  assert(before && before.value === from, "ramp opening value absent");
  const tail = rows.filter(row => row.ms >= start && row.ms <= start+4500);
  const target = tail.findIndex(row => row.value === to);
  assert(target >= 0, "ramp target absent");
  const ramp = tail.slice(0,target+1), direction = Math.sign(to-from);
  let previous = from;
  for (const row of ramp) { assert(direction*(row.value-previous) >= 0 && direction*(to-row.value) >= 0, "nonmonotonic ramp"); previous = row.value; }
  const distinct = new Set(ramp.map(row => row.value));
  const intermediate = [...distinct].filter(value => value !== from && value !== to).length;
  assert(intermediate >= 2, "endpoint jump is not a fade");
  const duration = ramp.at(-1).ms-start;
  assert(duration >= 2500 && duration <= 4000, "fade duration outside declared tolerance");
  return {from_value:from, to_value:to, intermediate_values:intermediate, target_delay_ms:duration, monotonic:1};
}

async function executeCommand(api, port, value) {
  const socket = new WebSocket(`ws://127.0.0.1:${port}/api/v2/events`, ["light.events.v2", `light.token.${api.session.token}`]);
  let timer;
  try {
    const response = await new Promise((resolve,reject) => {
      const requestId = crypto.randomUUID(); let sent = false;
      timer = setTimeout(() => reject(new Error("command action timeout")), 10_000);
      socket.addEventListener("error", () => reject(new Error("command socket error")));
      socket.addEventListener("close", () => reject(new Error("command socket closed before response")));
      socket.addEventListener("open", () => socket.send(JSON.stringify({type:"subscribe",filter:{capabilities:["system"]},capacity:32,rate_limits:[]})));
      socket.addEventListener("message", event => {
        try {
          const text = String(event.data); assert(Buffer.byteLength(text) <= 1024*1024, "command response cap");
          const row = JSON.parse(text);
          if (row.type === "ready" && !sent) {
            sent = true;
            socket.send(JSON.stringify({type:"action",protocol_version:2,request_id:requestId,session_id:api.session.session_id,action:{type:"command_line_execute",request:{value}}}));
          } else if (row.request_id === requestId) { assert(row.ok === true, "command rejected"); resolve(row); }
          else if (row.type === "gap" || row.type === "error") throw new Error("command subscription failed");
        } catch(error) { reject(error); }
      });
    });
    return response;
  } finally {
    clearTimeout(timer);
    await new Promise((resolve,reject) => {
      if (socket.readyState === WebSocket.CLOSED) return resolve();
      const limit = setTimeout(() => reject(new Error("command socket close timeout")), 2000);
      socket.addEventListener("close", () => {clearTimeout(limit);resolve();}, {once:true});
      socket.close();
    });
  }
}

export async function configurePacketProof(api, port) {
  const require = createRequire(path.join(ROOT,"package.json"));
  const source = await readFile(path.join(ROOT,"tests/bench/core/protocols.ts"),"utf8");
  assert.equal(sha(source), "face0cd976855cc8aa28432b6d21dc022fbc68668d94b7b66f28199fc3c8b1de");
  const compiled = await require("esbuild").transform(source,{loader:"ts",format:"esm",target:"node22",sourcemap:false});
  const {DmxReceiver} = await import(`data:text/javascript;base64,${Buffer.from(compiled.code).toString("base64")}`);
  const receiver = await DmxReceiver.bind();
  const destination = `127.0.0.1:${receiver.port}`;
  let timer, aggregate, cursor, closed = false;
  const commands = [];
  const drain = () => {
    if (!aggregate || aggregate.error) return;
    try {
      cursor = drainPacketRing(receiver,cursor,aggregate);
    } catch(error) { aggregate.error = error; }
  };
  const close = async () => { if (closed) return; closed = true; clearInterval(timer); drain(); await receiver.close(); };
  try {
    const routes = await api.request("GET","/api/v2/objects/route");
    for (const route of routes.objects) if (route.body.enabled) await api.request("POST","/api/v2/output-routes/actions",{request_id:crypto.randomUUID(),action:{type:"update",route_id:route.id,expected_revision:route.revision,patch:{enabled:false}}});
    await api.request("POST","/api/v2/output-routes/actions",{request_id:crypto.randomUUID(),action:{type:"create_range",range_id:crypto.randomUUID(),route:{target:{kind:"network"},protocol:"art_net",logical_universe:101,destination_universe:101,delivery_mode:"unicast",destination,enabled:true,minimum_slots:512},logical_universe_end:116,destination_universe_end:116}});
    const active = (await api.request("GET","/api/v2/objects/route")).objects.filter(route => route.body.enabled);
    assert.equal(active.length,16);
    assert.equal(new Set(active.map(route => route.body.logical_universe)).size,16);
    for (const {body} of active) assert(body.target?.kind === "network" && body.protocol === "art_net" && body.delivery_mode === "unicast" && body.destination === destination && body.minimum_slots === 512 && body.logical_universe >= 101 && body.logical_universe <= 116 && body.destination_universe === body.logical_universe,"route mismatch");
    assert.equal((await api.request("GET","/api/v2/configuration")).configuration.frame_rate_hz,40);
    const patch = await api.request("GET","/api/v2/patch");
    const fixture = patch.fixtures.find(f => f.fixture_number === 421);
    assert(fixture && fixture.split_patches?.length === 1 && fixture.split_patches[0].universe === 101 && fixture.split_patches[0].address === 511,"static fixture address changed");
    return {
      start() { assert(!aggregate); aggregate = new PacketAggregate(); cursor = receiver.mark(); timer = setInterval(drain,10); },
      snapshot() { drain(); const result = aggregate.snapshot(); assert(result.rows.every(row => row.packets > 0),"missing universe"); return result; },
      async interact() {
        drain(); assert.equal(aggregate.staticRows.at(-1)?.value,0,"static fixture not initially zero");
        const ramps = [];
        for (const [command,from,to] of [["FIXTURE 421 AT 50 TIME 3",0,128],["FIXTURE 421 AT 25 TIME 3",128,64]]) {
          const start = performance.now(); const response = await executeCommand(api,port,command); commands.push({command,start_monotonic_ms:start,response}); await sleep(4200); drain();
          if (aggregate.error) throw aggregate.error;
          ramps.push(summarizeRamp(aggregate.staticRows,start,from,to));
        }
        return {accepted_commands:2, ramps};
      },
      close,
      privateState() { return aggregate ? {packets:{packet_count:aggregate.count,rows:aggregate.rows},static_rows:aggregate.staticRows,commands,failed:Number(Boolean(aggregate.error))} : null; },
    };
  } catch(error) { await close(); throw error; }
}

async function reportRecords(file) {
  assert((await stat(file)).size <= 64*1024*1024,"private Stage report cap");
  const text = await readFile(file,"utf8");
  const lines = text.split("\n"); if (lines.at(-1) !== "") lines.pop();
  assert(lines.length <= 100_000,"private report row cap");
  return lines.filter(Boolean).map(line => JSON.parse(line));
}
async function waitRecord(file, kind, timeout, assertRunning) {
  const end = performance.now()+timeout;
  while(performance.now() < end) {
    assertRunning();
    let rows;
    try { rows = await reportRecords(file); } catch(error) { if (error.code !== "ENOENT") throw error; }
    const row = rows?.find(row => row.kind === kind);
    if (row) return row;
    await sleep(250);
  }
  throw new Error(`Stage ${kind} timed out`);
}
export const waitStageStarted = (file,assertRunning) => waitRecord(file,"stage-started",90_000,assertRunning);
export const waitStageComplete = (file,assertRunning) => waitRecord(file,"complete",300_000,assertRunning);

export function summarizeStageWindow(complete, start, end) {
  nonnegative(start); nonnegative(end); assert(end-start >= 60_000,"short Stage window");
  assert(complete.kind === "complete" && complete.measurementSurface === "packaged-tauri-native-stage");
  const frames = complete.nativeStage?.frames; assert(Array.isArray(frames) && frames.length <= 100_000,"Stage frame cap");
  const lanes = ["normal","preload"].map(lane => {
    const rows = frames.filter(frame => frame.lane === lane);
    assert(rows.length >= 2, "missing Stage lane");
    const times = [];
    for (const row of rows) {
      assert(typeof row.paneId === "string" && row.paneId.length > 0 && row.width > 0 && row.height > 0 && row.drawCalls > 0,"invalid Stage sample");
      assert(row.followPreload === (lane === "preload"),"Stage lane provenance mismatch");
      uint(row.sequence);uint(row.sourceFrame); nonnegative(row.presentedEpochMicros);
      times.push(row.presentedEpochMicros/1000);
    }
    times.sort((a,b) => a-b);
    assert(times[0] <= start && times.at(-1) >= end,"Stage samples do not bracket measurement");
    const inside = times.filter(t => t >= start && t <= end); assert(inside.length > 0,"no Stage samples in window");
    const edges = [start,...inside,end];
    const gap = Math.max(...edges.slice(1).map((t,i) => t-edges[i]));
    assert(gap <= 5000,"Stage presentation observation gap exceeds 5000ms");
    return {observed_samples:inside.length,max_gap_ms:gap,bracketed:1};
  });
  return {lanes,max_allowed_gap_ms:5000,retrospective:1};
}

const SCALARS = ["schema_version","complete","all_gates_passed","requested_hz","minimum_required_hz","counter_observations","interval_count","actual_window_seconds","completed_frames","weighted_completed_hz","minimum_interval_hz","mean_interval_hz","windows_below_38hz","deadline_misses","send_errors","packet_count","accepted_commands","stage_max_allowed_gap_ms","wall_monotonic_difference_ms","fixture_records","physical_instances","parameter_count","universe_count","animated_attributes","master_lanes"];
const RESOURCE_FIELDS = ["cpu_average_percent","cpu_max_percent","peak_resident_bytes"];
// UDP capture includes opening/closing request bounds whereas backend counter time uses
// midpoints. Predeclared allowance: at most two fewer/eight additional packets per universe.
// This tests decoded loopback delivery only, not sender identity or kernel/raw wire loss.
function gates(report) {
  return Boolean(report.complete && report.counter_observations === 61 && report.interval_count === 60 && report.actual_window_seconds >= 60 && report.minimum_interval_hz >= 38 && report.windows_below_38hz === 0 && report.deadline_misses === 0 && report.send_errors === 0 && report.accepted_commands === 2 && report.wall_monotonic_difference_ms <= 1000 && report.universes.every(row => row.packets >= report.completed_frames-2 && row.packets <= report.completed_frames+8 && row.packets/report.actual_window_seconds >= 38 && row.sequence_discontinuities === 0 && row.zero_sequence_packets === 0));
}
export function validateNumericProof(report) {
  assert.deepEqual(Object.keys(report).sort(), [...SCALARS,"resources","universes","ramps","stage_lanes"].sort(),"public closed shape");
  for (const name of SCALARS) nonnegative(report[name]);
  assert.equal(report.schema_version,1);
  assert([0,1].includes(report.complete) && [0,1].includes(report.all_gates_passed));
  assert.equal(report.requested_hz,40);assert.equal(report.minimum_required_hz,38);
  assert(Array.isArray(report.resources) && report.resources.length === 5);
  for (const row of report.resources) { assert.deepEqual(Object.keys(row).sort(),RESOURCE_FIELDS.slice().sort()); for (const value of Object.values(row)) nonnegative(value); }
  for (const [name,length,keys] of [["universes",16,["packets","sequence_discontinuities","zero_sequence_packets"]],["ramps",2,["from_value","to_value","intermediate_values","target_delay_ms","monotonic"]],["stage_lanes",2,["observed_samples","max_gap_ms","bracketed"]]]) {
    assert(Array.isArray(report[name]) && report[name].length === length,"public fixed rows");
    for (const row of report[name]) { assert.deepEqual(Object.keys(row).sort(),keys.slice().sort()); for (const value of Object.values(row)) nonnegative(value); }
  }
  assert.equal(report.packet_count, report.universes.reduce((sum,row) => sum+row.packets,0));
  for (const name of ["counter_observations","interval_count","completed_frames","windows_below_38hz","deadline_misses","send_errors","packet_count","accepted_commands","fixture_records","physical_instances","parameter_count","universe_count","animated_attributes","master_lanes"]) uint(report[name]);
  for (const row of report.universes) { for(const value of Object.values(row)) uint(value); assert(row.zero_sequence_packets <= row.packets && row.sequence_discontinuities <= row.packets); }
  if (report.complete) {
    assert.deepEqual([report.fixture_records,report.physical_instances,report.parameter_count,report.universe_count,report.animated_attributes,report.master_lanes],[576,576,8064,16,5408,20]);
    assert.equal(report.counter_observations,61); assert.equal(report.interval_count,60);
    assert(report.actual_window_seconds >= 60);
    assert(Math.abs(report.weighted_completed_hz-report.completed_frames/report.actual_window_seconds) <= 1e-9);
    assert(report.stage_lanes.every(row => row.observed_samples > 0 && row.bracketed === 1 && row.max_gap_ms <= 5000));
    assert.equal(report.stage_max_allowed_gap_ms,5000);
    assert(report.ramps.every(row => row.monotonic === 1 && row.intermediate_values >= 2 && row.target_delay_ms >= 2500 && row.target_delay_ms <= 4000));
    assert.deepEqual(report.ramps.map(row => [row.from_value,row.to_value]),[[0,128],[128,64]]);
  } else assert.equal(report.all_gates_passed,0);
  assert.equal(report.all_gates_passed, Number(gates(report)), "public gate truth mismatch");
  return report;
}
export function numericProof(result) {
  const output = Object.fromEntries(SCALARS.map(name => [name,0]));
  Object.assign(output,{schema_version:1,requested_hz:40,minimum_required_hz:38,
    resources:Array.from({length:5},() => Object.fromEntries(RESOURCE_FIELDS.map(name => [name,0]))),
    universes:Array.from({length:16},() => ({packets:0,sequence_discontinuities:0,zero_sequence_packets:0})),
    ramps:Array.from({length:2},() => ({from_value:0,to_value:0,intermediate_values:0,target_delay_ms:0,monotonic:0})),
    stage_lanes:Array.from({length:2},() => ({observed_samples:0,max_gap_ms:0,bracketed:0})),
  });
  if (result) {
    const r = result.resources;
    Object.assign(output,{
      complete:1,fixture_records:result.fixture_count,physical_instances:result.physical_instance_count,parameter_count:result.parameter_count,universe_count:result.universes,animated_attributes:result.animated_attribute_count,master_lanes:result.master_lane_count,counter_observations:result.counter_observation_count,interval_count:result.counter_observation_count-1,
      actual_window_seconds:result.counter_window_seconds,completed_frames:result.completed_frame_delta,
      weighted_completed_hz:result.completed_frame_delta/result.counter_window_seconds,
      minimum_interval_hz:result.minimum_one_second_completed_hz,mean_interval_hz:result.average_completed_hz,
      windows_below_38hz:result.windows_below_target,deadline_misses:result.deadline_miss_delta,send_errors:result.send_error_delta,
      packet_count:result.proof.packets.packet_count,universes:result.proof.packets.rows,
      accepted_commands:result.proof.interaction.accepted_commands,ramps:result.proof.interaction.ramps,
      stage_lanes:result.proof.stage.lanes,stage_max_allowed_gap_ms:result.proof.stage.max_allowed_gap_ms,
      wall_monotonic_difference_ms:result.wall_monotonic_difference_ms,
      resources:[{cpu_average_percent:r.application_cpu_average_percent,cpu_max_percent:r.application_cpu_max_percent,peak_resident_bytes:r.application_peak_resident_bytes},r.server,r.desktop_webview,r.native_renderer,r.desktop_host],
    });
    output.all_gates_passed = Number(gates(output));
  }
  return validateNumericProof(output);
}
export async function writeNumericProof(file,result) {
  const report = numericProof(result); await mkdir(path.dirname(file),{recursive:true});
  await writeFile(file,`${JSON.stringify(report,null,2)}\n`,{flag:"wx",mode:0o600}); return report;
}
