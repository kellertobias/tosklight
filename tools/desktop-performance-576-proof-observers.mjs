// Observers for the focused released Desk proof. Raw records never leave private scratch.
import assert from "node:assert/strict";
import crypto from "node:crypto";
import { readFile, stat, writeFile, mkdir, rename, unlink } from "node:fs/promises";
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
      async interact(checkpoint = async () => {}) {
        drain(); assert.equal(aggregate.staticRows.at(-1)?.value,0,"static fixture not initially zero");
        const ramps = [];
        for (const [command,from,to] of [["FIXTURE 421 AT 50 TIME 3",0,128],["FIXTURE 421 AT 25 TIME 3",128,64]]) {
          await checkpoint(ramps.length === 0 ? 9 : 10,{accepted_commands:commands.length,ramps:[...ramps]});
          const start = performance.now(); const response = await executeCommand(api,port,command); commands.push({command,start_monotonic_ms:start,response}); await checkpoint(ramps.length === 0 ? 9 : 10,{accepted_commands:commands.length,ramps:[...ramps]}); await sleep(4200); drain();
          if (aggregate.error) throw aggregate.error;
          ramps.push(summarizeRamp(aggregate.staticRows,start,from,to));
          await checkpoint(ramps.length === 1 ? 9 : 10,{accepted_commands:commands.length,ramps:[...ramps]});
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

const STAGE_INVALID_SCOPES = ["before", "in_window", "after", "unlocalizable"];
const STAGE_INVALID_REASONS = ["identity", "clock", "sequence_source", "dimensions", "content"];
const stageInvalidFields = prefix => [...STAGE_INVALID_SCOPES,...STAGE_INVALID_REASONS].map(name => `${prefix}_invalid_${name}_samples`);

// A missing lane or coverage gap is numeric diagnostic evidence, not an exception that
// discards the already-completed backend measurement. No pane identifier is exported.
export function summarizeStageWindow(complete, start, end) {
  nonnegative(start); nonnegative(end); assert(end-start >= 60_000,"short Stage window");
  const stage=complete?.nativeStage, activity=stage?.activity, heartbeats=stage?.heartbeats, filtered=stage?.frames;
  const overflow=stage?.observerOverflow;
  const shape=complete?.schemaVersion === 2 && complete?.kind === "complete" && complete?.measurementSurface === "packaged-tauri-native-stage" && Array.isArray(filtered) && filtered.length <= 100_000 && Array.isArray(activity) && activity.length <= 20_000 && Array.isArray(heartbeats) && heartbeats.length <= 10_000;
  const overflowValid=overflow && Object.keys(overflow).sort().join(",") === "activity,errors,heartbeats" && Object.values(overflow).every(value => Number.isSafeInteger(value) && value >= 0);
  const reportAvailable=Boolean(shape && overflowValid), observerFailed=!overflowValid || Object.values(overflow).some(value => value !== 0);
  const integer=value => Number.isSafeInteger(value) && value >= 0;
  const identity=row => typeof row?.paneId === "string" && row.paneId.startsWith("main:") && row.paneId.length <= 256 && typeof row.attachmentId === "string" && row.attachmentId.length > 0 && row.attachmentId.length <= 128;
  const key=row => `${row.paneId}\0${row.attachmentId}`;
  const closeClock=micros => integer(micros) && Math.abs(micros/1000-start) <= 400_000;
  const inWindow=millis => millis >= start && millis <= end;
  const frameChecks=row => ({identity:identity(row),clock:closeClock(row?.presentedEpochMicros),sequence_source:integer(row?.sequence) && integer(row?.sourceFrame),dimensions:integer(row?.width) && row?.width > 0 && integer(row?.height) && row?.height > 0,content:typeof row?.followPreload === "boolean" && integer(row?.drawCalls) && row?.drawCalls > 0});
  const heartbeatChecks=row => ({identity:identity(row),clock:closeClock(row?.epochMicros) && closeClock(row?.receivedEpochMicros),sequence_source:integer(row?.sequence) && integer(row?.lastPresentedSequence) && integer(row?.sourceFrame),dimensions:integer(row?.width) && integer(row?.height),content:typeof row?.followPreload === "boolean" && typeof row?.attached === "boolean" && typeof row?.visible === "boolean" && typeof row?.idleUnchanged === "boolean" && typeof row?.drawHealthy === "boolean"});
  const valid=checks => Object.values(checks).every(Boolean);
  const frameShape=row => valid(frameChecks(row));
  const heartbeatShape=row => valid(heartbeatChecks(row));
  const invalidDiagnostics=(rows,check,timeField,prefix) => {
    const counts=Object.fromEntries(stageInvalidFields(prefix).map(name => [name,0]));
    for(const row of rows) {
      const checks=check(row);if(valid(checks))continue;
      // Only a known owner and valid bounded clocks can establish an out-of-window row.
      // Missing identity/time remains a gate failure; it cannot be silently scoped away.
      const scope=!checks.identity || !checks.clock ? "unlocalizable" : row?.[timeField]/1000 < start ? "before" : row?.[timeField]/1000 > end ? "after" : "in_window";
      counts[`${prefix}_invalid_${scope}_samples`]++;
      // Reason buckets may overlap. Content means draw-call evidence for presentations,
      // or the explicit boolean state fields for heartbeats; no private values are emitted.
      for(const reason of STAGE_INVALID_REASONS)if(!checks[reason])counts[`${prefix}_invalid_${reason}_samples`]++;
    }
    return counts;
  };
  const coverage=times => {
    const ordered=[...times].sort((a,b) => a-b), inside=ordered.filter(inWindow), edges=[start,...inside,end];
    return {observed:inside.length,first:ordered.length ? ordered[0]-start : 0,last:ordered.length ? ordered.at(-1)-start : 0,begin:inside.length ? inside[0]-start : end-start,end:inside.length ? end-inside.at(-1) : end-start,max:Math.max(...edges.slice(1).map((t,i) => t-edges[i])),bracketed:Number(ordered.length > 0 && ordered[0] <= start && ordered.at(-1) >= end)};
  };
  const unknownLane=reportAvailable && [...activity,...heartbeats].some(row => typeof row?.followPreload !== "boolean");
  const lanes=[false,true].map(preload => {
    const all=reportAvailable ? activity.filter(row => row?.followPreload === preload) : [];
    const allHeartbeats=reportAvailable ? heartbeats.filter(row => row?.followPreload === preload) : [];
    const candidates=new Set();
    for(const row of all) if(frameShape(row) && inWindow(row.presentedEpochMicros/1000)) candidates.add(key(row));
    for(const row of allHeartbeats) if(heartbeatShape(row) && inWindow(row.receivedEpochMicros/1000)) candidates.add(key(row));
    const selected=[...candidates][0];
    const rows=all.filter(row => frameShape(row) && key(row) === selected);
    const invalid=all.filter(row => !frameShape(row)).length;
    const hbRows=allHeartbeats.filter(row => heartbeatShape(row) && key(row) === selected);
    const hbInvalid=allHeartbeats.filter(row => !heartbeatShape(row)).length;
    const invalidActivity=invalidDiagnostics(all,frameChecks,"presentedEpochMicros","activity");
    const invalidHeartbeats=invalidDiagnostics(allHeartbeats,heartbeatChecks,"receivedEpochMicros","heartbeat");
    const invalidWindow=invalidActivity.activity_invalid_in_window_samples+invalidActivity.activity_invalid_unlocalizable_samples+invalidHeartbeats.heartbeat_invalid_in_window_samples+invalidHeartbeats.heartbeat_invalid_unlocalizable_samples;
    let reordered=0, hbReordered=0, previous, previousHeartbeat;
    for(const row of rows) {
      if(previous && (row.sequence <= previous.sequence || row.presentedEpochMicros <= previous.presentedEpochMicros || row.sourceFrame < previous.sourceFrame)) reordered++;
      previous=row;
    }
    for(const row of hbRows) {
      if(previousHeartbeat && (row.sequence <= previousHeartbeat.sequence || row.epochMicros <= previousHeartbeat.epochMicros || row.receivedEpochMicros < previousHeartbeat.receivedEpochMicros || row.lastPresentedSequence < previousHeartbeat.lastPresentedSequence || row.sourceFrame < previousHeartbeat.sourceFrame)) hbReordered++;
      previousHeartbeat=row;
    }
    const bySequence=new Map(rows.map(row => [row.sequence,row]));
    let linkFailures=0, badIdle=0;
    const eligible=[];
    for(const hb of hbRows) {
      const frame=bySequence.get(hb.lastPresentedSequence);
      const linked=frame && hb.lastPresentedSequence > 0 && frame.presentedEpochMicros <= hb.epochMicros && frame.sourceFrame === hb.sourceFrame && frame.width === hb.width && frame.height === hb.height && frame.followPreload === hb.followPreload;
      const fresh=hb.receivedEpochMicros >= hb.epochMicros && hb.receivedEpochMicros-hb.epochMicros <= 5_000_000;
      if(inWindow(hb.receivedEpochMicros/1000) && (!linked || !fresh)) linkFailures++;
      const idle=Boolean(linked && fresh && hb.idleUnchanged && hb.drawHealthy && hb.attached && hb.visible && hb.width > 0 && hb.height > 0);
      if(idle) eligible.push(hb.receivedEpochMicros/1000);
      else if(inWindow(hb.receivedEpochMicros/1000)) badIdle++;
    }
    const sourceChanges=[];let lastSource;
    for(const row of rows) {if(lastSource === undefined || row.sourceFrame > lastSource) sourceChanges.push(row.presentedEpochMicros/1000);lastSource=row.sourceFrame;}
    const source=coverage(sourceChanges);
    const sourcePassed=Number(sourceChanges.length >= 2 && source.observed > 0 && source.bracketed && source.max <= 5000);
    const presentation=coverage(rows.map(row => row.presentedEpochMicros/1000)), heartbeat=coverage(eligible);
    // Bracketing and source continuity use only fully valid rows above: an invalid warmup
    // or terminal row never supplies an anchor. Preserve its total/reason diagnostics while
    // judging invalid evidence against the actual measured window.
    const ownerValid=candidates.size === 1 && !unknownLane && invalidWindow === 0 && reordered === 0 && hbReordered === 0 && !observerFailed;
    const presentationPassed=Number(ownerValid && presentation.observed > 0 && presentation.bracketed && presentation.max <= 5000 && (preload || sourcePassed));
    const idlePassed=Number(ownerValid && preload && rows.length > 0 && badIdle === 0 && linkFailures === 0 && heartbeat.observed > 0 && heartbeat.bracketed && heartbeat.max <= 5000);
    return {
      available:Number(rows.length > 0),validation_passed:Number(Boolean(presentationPassed || idlePassed)),
      retained_samples:rows.length+invalid,valid_samples:rows.length,invalid_samples:invalid,
      ...invalidActivity,...invalidHeartbeats,
      all_activity_samples:all.length,filtered_samples:reportAvailable ? filtered.filter(row => row?.followPreload === preload).length : 0,
      first_sample_offset_ms:presentation.first,last_sample_offset_ms:presentation.last,observed_samples:presentation.observed,begin_gap_ms:presentation.begin,end_gap_ms:presentation.end,max_gap_ms:presentation.max,bracketed:presentation.bracketed,
      attachment_count:candidates.size,activity_reordered_samples:reordered,presentation_continuity_passed:presentationPassed,
      source_change_samples:Math.max(0,sourceChanges.length-1),source_observed_advances:sourceChanges.slice(1).filter(inWindow).length,source_begin_gap_ms:source.begin,source_end_gap_ms:source.end,source_max_gap_ms:source.max,source_bracketed:source.bracketed,source_continuity_passed:sourcePassed,idle_continuity_passed:idlePassed,
      heartbeat_samples:allHeartbeats.length,heartbeat_valid_samples:hbRows.length,heartbeat_invalid_samples:hbInvalid,heartbeat_reordered_samples:hbReordered,heartbeat_link_failures:linkFailures,heartbeat_ineligible_samples:badIdle,
      idle_heartbeat_samples:eligible.length,heartbeat_observed_samples:heartbeat.observed,heartbeat_begin_gap_ms:heartbeat.begin,heartbeat_end_gap_ms:heartbeat.end,heartbeat_max_gap_ms:heartbeat.max,heartbeat_bracketed:heartbeat.bracketed,
      observer_failed:Number(Boolean(observerFailed || unknownLane)),
    };
  });
  return {report_available:Number(reportAvailable),validation_passed:Number(reportAvailable && lanes.every(row => row.validation_passed === 1)),lanes,max_allowed_gap_ms:5000,retrospective:1};
}

// Phase codes are assigned only at trusted harness checkpoints, never inferred from errors:
// 0 finished/no exception;1 preflight;2 launch/backend;3 seed;4 routes;5 Sheet ready;
// 6 Stage start+17s;7 measurement;8 UDP;9 fade1;10 fade2;11 Sheet live;
// 12 Stage complete wait;13 Stage coverage;14 cleanup/evidence;15 outer failure.
export const FAILURE_STAGE_CODES = Object.freeze(Array.from({length:16},(_,i) => i));
function phase(value) { uint(value); assert(FAILURE_STAGE_CODES.includes(value),"unknown phase code"); return value; }
const FLAGS = ["complete","all_gates_passed","workload_available","measurement_available","measurement_passed","resources_available","udp_available","udp_passed","interaction_available","interaction_passed","sheet_liveness_checked","sheet_liveness_passed","stage_report_available","stage_validation_passed","cleanup_complete"];
const METRICS = ["requested_hz","minimum_required_hz","counter_observations","interval_count","actual_window_seconds","completed_frames","weighted_completed_hz","minimum_interval_hz","mean_interval_hz","windows_below_38hz","deadline_misses","send_errors","packet_count","accepted_commands","stage_max_allowed_gap_ms","wall_monotonic_difference_ms","fixture_records","physical_instances","parameter_count","universe_count","animated_attributes","master_lanes"];
const SCALARS = ["schema_version","current_stage_code","failure_stage_code",...FLAGS,...METRICS];
const RESOURCE_FIELDS = ["cpu_average_percent","cpu_max_percent","peak_resident_bytes"];
const LANE_FIELDS = [...stageInvalidFields("activity"),...stageInvalidFields("heartbeat"),"retained_samples","first_sample_offset_ms","last_sample_offset_ms","available","validation_passed","valid_samples","invalid_samples","observed_samples","begin_gap_ms","end_gap_ms","max_gap_ms","bracketed","all_activity_samples","filtered_samples","attachment_count","activity_reordered_samples","presentation_continuity_passed","idle_continuity_passed","heartbeat_samples","heartbeat_valid_samples","heartbeat_invalid_samples","heartbeat_reordered_samples","heartbeat_link_failures","heartbeat_ineligible_samples","idle_heartbeat_samples","heartbeat_observed_samples","heartbeat_begin_gap_ms","heartbeat_end_gap_ms","heartbeat_max_gap_ms","heartbeat_bracketed","observer_failed","source_change_samples","source_observed_advances","source_begin_gap_ms","source_end_gap_ms","source_max_gap_ms","source_bracketed","source_continuity_passed"];
const RAMP_FIELDS = ["available","from_value","to_value","intermediate_values","target_delay_ms","monotonic"];
const MEASUREMENT_FIELDS = ["counter_observations","interval_count","actual_window_seconds","completed_frames","weighted_completed_hz","minimum_interval_hz","mean_interval_hz","windows_below_38hz","deadline_misses","send_errors","wall_monotonic_difference_ms"];
function measurementPass(r) { return Boolean(r.measurement_available && r.counter_observations === 61 && r.interval_count === 60 && r.actual_window_seconds >= 60 && r.minimum_interval_hz >= 38 && r.windows_below_38hz === 0 && r.deadline_misses === 0 && r.send_errors === 0 && r.wall_monotonic_difference_ms <= 1000); }
// Decoded loopback only: midpoint/envelope allowance is two fewer/eight more packets.
function udpPass(r) { return Boolean(r.udp_available && r.measurement_available && r.actual_window_seconds > 0 && r.universes.every(row => row.packets >= r.completed_frames-2 && row.packets <= r.completed_frames+8 && row.packets/r.actual_window_seconds >= 38 && row.sequence_discontinuities === 0 && row.zero_sequence_packets === 0)); }
function interactionPass(r) { return Boolean(r.interaction_available && r.accepted_commands === 2 && r.ramps.every(row => row.available && row.monotonic === 1 && row.intermediate_values >= 2 && row.target_delay_ms >= 2500 && row.target_delay_ms <= 4000) && r.ramps[0].from_value === 0 && r.ramps[0].to_value === 128 && r.ramps[1].from_value === 128 && r.ramps[1].to_value === 64); }
function gates(r) { return Boolean(r.complete && r.failure_stage_code === 0 && r.workload_available && r.measurement_passed && r.resources_available && r.udp_passed && r.interaction_passed && r.sheet_liveness_checked && r.sheet_liveness_passed && r.stage_report_available && r.stage_validation_passed && r.cleanup_complete); }
function closed(row,keys) { assert(row && !Array.isArray(row)); assert.deepEqual(Object.keys(row).sort(),keys.slice().sort(),"public closed shape"); for(const [name,value] of Object.entries(row)) {if(name === "first_sample_offset_ms" || name === "last_sample_offset_ms") assert(typeof value === "number" && Number.isFinite(value) && Math.abs(value) <= 400_000,"invalid bounded offset");else nonnegative(value);} }
function bit(value) { assert(value === 0 || value === 1,"invalid boolean flag"); }
export function validateNumericProof(r) {
  closed(Object.fromEntries(Object.entries(r).filter(([name]) => !["resources","universes","ramps","stage_lanes"].includes(name))),SCALARS);
  assert.deepEqual(Object.keys(r).sort(),[...SCALARS,"resources","universes","ramps","stage_lanes"].sort(),"public closed shape");
  assert.equal(r.schema_version,3); phase(r.current_stage_code);phase(r.failure_stage_code);
  for(const name of FLAGS) bit(r[name]);
  assert.equal(r.requested_hz,40);assert.equal(r.minimum_required_hz,38);
  for (const [name,length,keys] of [["resources",5,RESOURCE_FIELDS],["universes",16,["packets","sequence_discontinuities","zero_sequence_packets"]],["ramps",2,RAMP_FIELDS],["stage_lanes",2,LANE_FIELDS]]) {
    assert(Array.isArray(r[name]) && r[name].length === length,"public fixed rows");
    for(const row of r[name]) closed(row,keys);
  }
  for(const name of ["counter_observations","interval_count","completed_frames","windows_below_38hz","deadline_misses","send_errors","packet_count","accepted_commands","fixture_records","physical_instances","parameter_count","universe_count","animated_attributes","master_lanes"]) uint(r[name]);
  for(const row of r.universes) {for(const value of Object.values(row)) uint(value);assert(row.zero_sequence_packets <= row.packets && row.sequence_discontinuities <= row.packets);}
  assert.equal(r.packet_count,r.universes.reduce((n,row) => n+row.packets,0));
  if(r.workload_available) assert.deepEqual([r.fixture_records,r.physical_instances,r.parameter_count,r.universe_count,r.animated_attributes,r.master_lanes],[576,576,8064,16,5408,20]);
  else assert([r.fixture_records,r.physical_instances,r.parameter_count,r.universe_count,r.animated_attributes,r.master_lanes].every(value => value === 0));
  if(r.measurement_available) {
    assert.equal(r.counter_observations,61);assert.equal(r.interval_count,60);assert(r.actual_window_seconds >= 60);
    assert(Math.abs(r.weighted_completed_hz-r.completed_frames/r.actual_window_seconds) <= 1e-9);
  } else assert(MEASUREMENT_FIELDS.every(name => r[name] === 0),"unavailable measurement has values");
  if(!r.resources_available) assert(r.resources.every(row => Object.values(row).every(value => value === 0)));
  if(!r.udp_available) assert(r.packet_count === 0 && r.universes.every(row => Object.values(row).every(value => value === 0)));
  for(const row of r.ramps) {bit(row.available);bit(row.monotonic);if(!row.available) assert(Object.values(row).every(value => value === 0));}
  assert.equal(r.interaction_available,Number(r.ramps.some(row => row.available)));
  assert(r.accepted_commands <= 2);
  for(const [laneIndex,row] of r.stage_lanes.entries()) {
    for(const name of ["available","validation_passed","bracketed","presentation_continuity_passed","idle_continuity_passed","heartbeat_bracketed","observer_failed","source_bracketed","source_continuity_passed"]) bit(row[name]);
    for(const name of ["retained_samples","valid_samples","invalid_samples","observed_samples"]) uint(row[name]);
    assert(row.observed_samples <= row.valid_samples);assert.equal(row.retained_samples,row.valid_samples+row.invalid_samples);
    assert.equal(row.available,Number(row.valid_samples > 0));
    if(row.available) assert(row.first_sample_offset_ms <= row.last_sample_offset_ms);else assert(row.first_sample_offset_ms === 0 && row.last_sample_offset_ms === 0);
    for(const name of ["all_activity_samples","filtered_samples","attachment_count","activity_reordered_samples","heartbeat_samples","heartbeat_valid_samples","heartbeat_invalid_samples","heartbeat_reordered_samples","heartbeat_link_failures","heartbeat_ineligible_samples","idle_heartbeat_samples","heartbeat_observed_samples","source_change_samples","source_observed_advances"]) uint(row[name]);
    assert(row.retained_samples <= row.all_activity_samples && row.heartbeat_observed_samples <= row.idle_heartbeat_samples && row.idle_heartbeat_samples <= row.heartbeat_valid_samples && row.heartbeat_valid_samples+row.heartbeat_invalid_samples <= row.heartbeat_samples);
    if(row.source_continuity_passed) assert(row.source_change_samples > 0 && row.source_observed_advances > 0 && row.source_bracketed && row.source_max_gap_ms <= 5000);
    if(row.presentation_continuity_passed) assert(row.available && row.observed_samples > 0 && row.bracketed && row.max_gap_ms <= 5000 && (laneIndex === 1 || row.source_continuity_passed));
    if(row.idle_continuity_passed) assert(laneIndex === 1 && row.available && row.heartbeat_observed_samples > 0 && row.heartbeat_bracketed && row.heartbeat_max_gap_ms <= 5000 && row.heartbeat_ineligible_samples === 0 && row.heartbeat_link_failures === 0);
    assert.equal(row.validation_passed,Number(Boolean(row.presentation_continuity_passed || row.idle_continuity_passed)));
    for(const [prefix,total,cap] of [["activity",row.invalid_samples,20_000],["heartbeat",row.heartbeat_invalid_samples,10_000]]) {
      assert(total <= cap);
      for(const name of stageInvalidFields(prefix)){uint(row[name]);assert(row[name] <= total && row[name] <= cap);}
      assert.equal(STAGE_INVALID_SCOPES.reduce((sum,name) => sum+row[`${prefix}_invalid_${name}_samples`],0),total,"invalid scope conservation");
      assert(STAGE_INVALID_REASONS.reduce((sum,name) => sum+row[`${prefix}_invalid_${name}_samples`],0) >= total,"invalid reason coverage");
    }
    if(row.validation_passed) assert(row.attachment_count === 1 && row.activity_invalid_in_window_samples === 0 && row.activity_invalid_unlocalizable_samples === 0 && row.heartbeat_invalid_in_window_samples === 0 && row.heartbeat_invalid_unlocalizable_samples === 0 && row.activity_reordered_samples === 0 && row.heartbeat_reordered_samples === 0 && row.observer_failed === 0);
  }
  assert.equal(r.stage_validation_passed,Number(Boolean(r.stage_report_available && r.stage_lanes.every(row => row.validation_passed))));
  if(!r.stage_report_available) assert(r.stage_validation_passed === 0);
  if(r.stage_report_available) assert.equal(r.stage_max_allowed_gap_ms,5000);
  assert(r.sheet_liveness_passed <= r.sheet_liveness_checked);
  assert.equal(r.measurement_passed,Number(measurementPass(r)));
  assert.equal(r.udp_passed,Number(udpPass(r)));
  assert.equal(r.interaction_passed,Number(interactionPass(r)));
  if(r.failure_stage_code) assert.equal(r.complete,0);
  if(r.complete) assert(r.current_stage_code === 0 && r.workload_available && r.measurement_available && r.resources_available && r.udp_available && r.interaction_available && r.sheet_liveness_checked && r.stage_report_available && r.cleanup_complete);
  assert.equal(r.all_gates_passed,Number(gates(r)),"public gate truth mismatch");
  return r;
}
export function numericProof(result,progress = {}) {
  const r = Object.fromEntries(SCALARS.map(name => [name,0]));
  Object.assign(r,{schema_version:3,current_stage_code:1,requested_hz:40,minimum_required_hz:38,
    resources:Array.from({length:5},() => Object.fromEntries(RESOURCE_FIELDS.map(name => [name,0]))),
    universes:Array.from({length:16},() => ({packets:0,sequence_discontinuities:0,zero_sequence_packets:0})),
    ramps:Array.from({length:2},() => Object.fromEntries(RAMP_FIELDS.map(name => [name,0]))),
    stage_lanes:Array.from({length:2},() => Object.fromEntries(LANE_FIELDS.map(name => [name,0]))),
  });
  if(result?.fixture_count !== undefined) Object.assign(r,{workload_available:1,fixture_records:result.fixture_count,physical_instances:result.physical_instance_count,parameter_count:result.parameter_count,universe_count:result.universes,animated_attributes:result.animated_attribute_count,master_lanes:result.master_lane_count});
  if(result?.counter_observation_count !== undefined) Object.assign(r,{measurement_available:1,counter_observations:result.counter_observation_count,interval_count:result.counter_observation_count-1,actual_window_seconds:result.counter_window_seconds,completed_frames:result.completed_frame_delta,weighted_completed_hz:result.completed_frame_delta/result.counter_window_seconds,minimum_interval_hz:result.minimum_one_second_completed_hz,mean_interval_hz:result.average_completed_hz,windows_below_38hz:result.windows_below_target,deadline_misses:result.deadline_miss_delta,send_errors:result.send_error_delta,wall_monotonic_difference_ms:result.wall_monotonic_difference_ms});
  if(result?.resources) {const v=result.resources;Object.assign(r,{resources_available:1,resources:[{cpu_average_percent:v.application_cpu_average_percent,cpu_max_percent:v.application_cpu_max_percent,peak_resident_bytes:v.application_peak_resident_bytes},v.server,v.desktop_webview,v.native_renderer,v.desktop_host]});}
  if(result?.proof?.packets) Object.assign(r,{udp_available:1,packet_count:result.proof.packets.packet_count,universes:result.proof.packets.rows});
  if(result?.proof?.interaction) {const v=result.proof.interaction;r.accepted_commands=v.accepted_commands;for(let i=0;i<v.ramps.length;i++)r.ramps[i]={available:1,...v.ramps[i]};r.interaction_available=Number(v.ramps.length>0);}
  if(result?.proof?.stage) {const v=result.proof.stage;Object.assign(r,{stage_report_available:v.report_available,stage_validation_passed:v.validation_passed,stage_lanes:v.lanes,stage_max_allowed_gap_ms:v.max_allowed_gap_ms});}
  // Completion, Sheet liveness and cleanup are trusted observations, never inferred
  // merely from a result object's presence. Callers must supply them explicitly.
  Object.assign(r,{complete:0,current_stage_code:1,sheet_liveness_checked:0,sheet_liveness_passed:0,cleanup_complete:0},progress);
  r.measurement_passed=Number(measurementPass(r));r.udp_passed=Number(udpPass(r));r.interaction_passed=Number(interactionPass(r));r.all_gates_passed=Number(gates(r));
  return validateNumericProof(r);
}
async function atomicNumericWrite(file,report) {
  await mkdir(path.dirname(file),{recursive:true});
  const temporary = `${file}.checkpoint-${crypto.randomUUID()}.tmp`;
  try {await writeFile(temporary,`${JSON.stringify(report,null,2)}\n`,{flag:"wx",mode:0o600});await rename(temporary,file);}
  catch(error) {await unlink(temporary).catch(() => {});throw error;}
}
export async function writeNumericProof(file,result,progress) {const report=numericProof(result,progress);await atomicNumericWrite(file,report);return report;}
export class ProofCheckpoint {
  constructor(file,io = {}) {
    this.file=file;this.write=io.write ?? atomicNumericWrite;this.result={proof:{}};
    this.progress={complete:0,current_stage_code:1,failure_stage_code:0,sheet_liveness_checked:0,sheet_liveness_passed:0,cleanup_complete:0};
    this.pending=Promise.resolve();
  }
  save() {
    const report=structuredClone(numericProof(this.result,this.progress));
    const next=this.pending.then(() => this.write(this.file,report));
    this.pending=next.catch(() => {});return next.then(() => report);
  }
  enter(code) {phase(code);assert(code !== 0);this.progress.current_stage_code=code;this.progress.complete=0;return this.save();}
  fail(code=this.progress.current_stage_code) {phase(code);assert(code !== 0);this.progress.failure_stage_code ||= code;this.progress.complete=0;return this.save();}
  finish(cleanupPassed) {bit(cleanupPassed);this.progress.cleanup_complete=cleanupPassed;if(!cleanupPassed)this.progress.failure_stage_code ||= 14;this.progress.complete=Number(!this.progress.failure_stage_code && cleanupPassed);if(this.progress.complete)this.progress.current_stage_code=0;return this.save();}
}
