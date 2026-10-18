import test from "node:test";
import assert from "node:assert/strict";
import {mkdtemp,readFile,stat,readdir,rm} from "node:fs/promises";
import path from "node:path";
import {artifactPaths} from "./artifact-paths.mjs";
import {ThreadCpuDiagnostics,threadCpuEnabled,verifiedClockTicks,parseTaskStat,taskRole,validateThreadCpuDiagnostics,writeThreadCpuDiagnostics} from "./desktop-performance-576-thread-cpu.mjs";
const epoch=1_700_000_000_000;
function text(tid,comm,user=0,system=0,start=tid){const fields=Array(22).fill("0");fields[0]="S";fields[1]="100";fields[11]=String(user);fields[12]=String(system);fields[19]=String(start);return `${tid} (${comm}) ${fields.join(" ")}\n`;}
const snapshot=[{pid:100,parentPid:1,startTicks:100,command:"desk"},{pid:200,parentPid:100,startTicks:200,command:"light-headless"}];
function mock({tasks=i=>[[200,"light-headless"],[201,"light-output-0"],[202,"light-output-1"],[203,"tokio-rt-worker"]],fault=()=>{},hz=100}={}){
 let index=0,clock=0,reads=0,lists=0,links=0;
 const io={async readFile(file){reads++;const fail=fault({index,file,kind:"read",reads,lists,links});if(fail instanceof Error)throw fail;if(typeof fail==="string")return fail;
  if(file==="/proc/100/stat")return text(100,"desk");if(file==="/proc/200/stat")return text(200,"light-headless",index*10,index*2);
  const tid=Number(/task\/(\d+)\/stat$/u.exec(file)?.[1]);const task=tasks(index).find(t=>t[0]===tid);assert(task);return text(tid,task[1],index*(tid===201?3:1),index,task[2]??tid);
 },async readdir(file){lists++;assert.equal(file,"/proc/200/task");const fail=fault({index,file,kind:"list",reads,lists,links});return fail??tasks(index).map(t=>String(t[0]));},async readlink(file){links++;assert.equal(file,"/proc/200/exe");return fault({index,file,kind:"link",reads,lists,links})??"/private/owned/light-headless";},async stat(file){assert.equal(file,"/proc/200/exe");return {dev:1n,ino:2n,size:100n,mtimeNs:3n,ctimeNs:4n,isFile:()=>true};}};
 const collector=new ThreadCpuDiagnostics(hz,{io,now:()=>{clock+=0.01;return clock;},utc:()=>epoch+Math.floor(clock)});
 return {collector,set(i){index=i;clock=i*1000+20;},sample(i){return {diagnostics:{output_sample:{sampled_at_epoch_micros:(epoch+i*1000)*1000,monotonic_micros:i*1e6}}};},get reads(){return reads;}};
}
async function collect(options={},n=61){const m=mock(options);for(let i=0;i<n;i++){m.set(i);await m.collector.observe(snapshot,100,m.sample(i));}return m;}
test("opt-in is fulltrace Control only; real CLK_TCK failure never becomes verified fallback100",()=>{
 for(const mode of ["stage-functional","combined"])assert.equal(threadCpuEnabled({LIGHT_OUTPUT_THREAD_CPU_DIAGNOSTICS:"1",LIGHT_OUTPUT_PHASE_DIAGNOSTICS:"all"},mode),false);
 for(const phase of [undefined,"true","false"])assert.equal(threadCpuEnabled({LIGHT_OUTPUT_THREAD_CPU_DIAGNOSTICS:"1",LIGHT_OUTPUT_PHASE_DIAGNOSTICS:phase},"control-no-stage"),false);
 assert.equal(threadCpuEnabled({LIGHT_OUTPUT_THREAD_CPU_DIAGNOSTICS:"1",LIGHT_OUTPUT_PHASE_DIAGNOSTICS:"all"},"control-no-stage"),true);
 assert.equal(verifiedClockTicks({status:0,stdout:"100\n"}),100);for(const r of [{status:1,stdout:"100"},{status:0,stdout:"100junk"},{status:0,stdout:"0"},{status:0,stdout:""},{status:0,stdout:"1000001"},{}])assert.equal(verifiedClockTicks(r),null);
});
test("strict proc parser preserves user/system/start and handles comm parentheses without private exports",()=>{
 assert.deepEqual(parseTaskStat(text(7,"a b)c",2,3,99)),{tid:7,comm:"a b)c",userTicks:2,systemTicks:3,startTicks:99});
 for(const bad of ["",text(7,"x".repeat(16)),text(7,"x",-1),text(7,"x",Number.MAX_SAFE_INTEGER+1),text(7,"x").replace(" 99 "," 1x "),"x".repeat(8193)]){if(bad===text(7,"x"))continue;assert.throws(()=>parseTaskStat(bad));}
 assert.equal(taskRole({tid:200,comm:"tokio-rt-worker"},200),0);assert.equal(taskRole({tid:201,comm:"light-output-0"},200),1);assert.equal(taskRole({tid:201,comm:"tokio-rt-worker"},200),2);assert.equal(taskRole({tid:201,comm:"light-programme"},200),3);assert.equal(taskRole({tid:201,comm:"light-output-sc"},200),4);
});
test("61 bounded censuses yield weighted stable-TID CPU with explicit approximate interval and numeric-only roles",async()=>{
 const m=await collect(),r=m.collector.summary();assert.equal(r.available,1);assert.equal(r.valid_intervals,60);assert.equal(r.tick_quantization_micros,10000);assert.equal(r.roles[1].user_ticks,240);assert.equal(r.roles[1].system_ticks,120);assert.equal(r.roles[1].cpu_micros,3_600_000);assert(Math.abs(r.roles[1].cpu_percent-6)<1e-6);assert.equal(r.server_window_micros,60_000_000);assert(Math.abs(r.window_micros-60_000_000)<=1);assert(r.minimum_window_micros<r.window_micros);assert(r.maximum_window_micros>r.window_micros);assert(r.census_total_micros>r.census_max_micros);assert.equal(r.output_caller_role_available,0);assert.equal(r.cleanup_role_available,0);assert.equal(r.pool_includes_cleanup_sort_search,1);
 assert.deepEqual(validateThreadCpuDiagnostics(JSON.parse(JSON.stringify(r))),r);for(const value of ["light-output","light-headless","/proc","/private",String(epoch)])assert(!JSON.stringify(r).includes(value));assert(m.collector.privateState().records[0].rows.some(row=>row.tid===201));
});
test("missing or unknown HZ is explicit unavailable, never fabricated measured CPUzero",async()=>{const r=(await collect({hz:null})).collector.summary();assert.equal(r.capture_complete,1);assert.equal(r.available,0);assert.equal(r.cpu_available,0);assert.equal(r.hz_verified,0);assert(r.roles[1].user_ticks>0);assert.equal(r.roles[1].cpu_micros,0);});
test("partial startup and extra observations fail closed",async()=>{
 const initial=mock().collector.summary();assert.equal(initial.available,0);assert.equal(initial.observations,0);
 for(const n of [1,60,62]){const r=(await collect({},n)).collector.summary();assert.equal(r.available,0);assert.equal(r.capture_complete,0);if(n===62)assert.equal(r.failure_code,4);}
});
test("birth, exit, TID reuse and comm rename exclude ambiguous intervalCPU and flag partialcoverage",async()=>{
 for(const kind of ["new","exit","reuse","rename"]){const m=await collect({tasks:i=>{const rows=[[200,"light-headless"],[201,"light-output-0"]];if(kind==="new"&&i>30)rows.push([202,"tokio-rt-worker"]);if(kind==="exit"&&i>30)rows.pop();if(kind==="reuse"&&i>30)rows[1][2]=900;if(kind==="rename"&&i>30)rows[1][1]="other";return rows;}});const r=m.collector.summary();assert.equal(r.available,0);assert.equal(r.capture_complete,1);assert.equal(r.partial_coverage,1);assert.equal(r[{new:"new_threads",exit:"exited_threads",reuse:"reused_threads",rename:"renamed_threads"}[kind]],1);assert(r.roles[1].matched_intervals<60||kind==="new");}
});
test("tasklist instability, PID/root reuse, executable change and IOerrors are unavailable",async()=>{
 const faults=[x=>x.kind==="list"&&x.lists===2?["200"]:undefined,x=>x.file==="/proc/200/stat"?text(200,"light-headless",0,0,999):undefined,x=>x.file==="/proc/100/stat"?text(100,"desk",0,0,999):undefined,x=>x.kind==="link"&&x.links===2?"/changed/exe":undefined,x=>x.kind==="read"&&x.reads===2?new Error("private failure"):undefined];
 for(const fault of faults){const m=await collect({fault},1),r=m.collector.summary();assert.equal(r.available,0);assert.equal(r.valid_observations,0);assert.equal(r.unreadable_censuses,1);assert(r.failure_code>0);assert(!JSON.stringify(r).includes("private failure"));}
});
test("task caps and malformed task IDs fail before unbounded task reads",async()=>{
 for(const ids of [Array.from({length:257},(_,i)=>String(i+1)),["200","200"],["200","1x"],[]]){const m=await collect({fault:x=>x.kind==="list"?ids:undefined},1);assert.equal(m.collector.summary().available,0);assert(m.reads<=2);}
});
test("counter decreases flag incomplete coverage, preserving no negative or recycled CPU",async()=>{
 const m=await collect({fault:x=>x.index===31&&x.file==="/proc/200/task/201/stat"?text(201,"light-output-0",0,0):undefined});const r=m.collector.summary();assert.equal(r.available,0);assert.equal(r.counter_failures,1);assert.equal(r.cpu_available,0);assert.equal(r.roles[1].user_ticks,150);assert.equal(r.roles[1].system_ticks,90);assert.equal(r.roles[1].matched_intervals,90);assert(r.roles.every(row=>row.user_ticks>=0&&row.system_ticks>=0));
});
test("role OTHER is actual valid CPU, not a separate fabricated scheduler or cleanup role",async()=>{
 const r=(await collect({tasks:()=>[[200,"light-headless"],[201,"unknown-thread"]]})).collector.summary();assert.equal(r.available,1);assert.equal(r.unresolved_threads,1);assert.equal(r.roles[4].matched_intervals,60);assert(r.roles[4].cpu_micros>0);assert.equal(r.output_caller_role_available,0);assert.equal(r.cleanup_role_available,0);
});
test("closed validator rejects metadata/coverage/counter/derived-metric forgeries",async()=>{
 const good=(await collect()).collector.summary();for(const mutate of [r=>r.pid=200,r=>r.available=0,r=>r.hz_verified=0,r=>r.roles[1].cpu_percent++,r=>r.roles[1].comm="secret",r=>r.roles[1].user_ticks=-1,r=>r.capture_complete=0,r=>r.exact_server_window=1,r=>r.roles[1].id=4,r=>r.unreadable_censuses=1]){const r=structuredClone(good);mutate(r);assert.throws(()=>validateThreadCpuDiagnostics(r));}
});
test("atomic public writer creates0600 numeric report only and no raw identity data",async()=>{
 const directory=await mkdtemp(path.join(artifactPaths.tmp,"thread-cpu-test-"));try{const file=path.join(directory,"thread-cpu-numeric.json"),m=await collect();await writeThreadCpuDiagnostics(file,m.collector);const r=JSON.parse(await readFile(file,"utf8"));assert.equal(r.available,1);assert.equal((await stat(file)).mode&0o777,0o600);assert.deepEqual(await readdir(directory),["thread-cpu-numeric.json"]);}finally{await rm(directory,{recursive:true,force:true});}
});

test("read brackets and task/clock invalidity fail closed without live processes",async()=>{
 const m=mock();m.set(0);await m.collector.observe(snapshot,100,m.sample(0));m.set(1);const bad=m.sample(1);bad.diagnostics.output_sample.sampled_at_epoch_micros+=100_000;await m.collector.observe(snapshot,100,bad);assert.equal(m.collector.summary().available,0);assert.equal(m.collector.summary().failure_code,5);
 const wrong=await collect({fault:x=>x.file==="/proc/200/task/201/stat"?text(202,"light-output-0"):undefined},1);assert.equal(wrong.collector.summary().failure_code,3);
 const slow=mock();slow.set(0);slow.collector.now=(()=>{let n=0;return ()=>n+=1000;})();await slow.collector.observe(snapshot,100,slow.sample(0));assert.equal(slow.collector.summary().failure_code,9);
 assert.equal(taskRole({tid:201,comm:"light-output-99"},200),1);assert.equal(taskRole({tid:201,comm:"light-output-01"},200),4);
});
test("runner uses optional observer at exactly61 existing process snapshots and issues no extra health calls",async()=>{
 const {ServerCounterClock}=await import("./desktop-performance-576-clock.mjs");const source=await readFile(new URL("./run-desktop-performance-576-proof.mjs",import.meta.url),"utf8");const body=source.slice(source.indexOf("async function measureWindow("),source.indexOf("\nfunction apiClient("));
 for(const enabled of [false,true]){let calls=0,proc=0,observations=0,wall=0;const samples=[];
 const capture=async()=>{const i=calls++,at=epoch+i*1000;return {diagnosticsUtcStartMillis:at-2,diagnosticsUtcEndMillis:at+2,diagnosticsMonotonicStartMillis:100+i*1000-2,diagnosticsMonotonicEndMillis:100+i*1000+2,diagnostics:{output_sample:{sampled_at_epoch_micros:at*1000+500,monotonic_micros:i*1e6+500},output:{frames_sent:i*40,packets_sent:i*640,deadline_misses:0,send_errors:0}}};};
 const make=new Function("capturePerformanceSample","readLinuxProcessTree","processTreeResourceDelta","performance","setTimeout","DURATION_SECONDS","LINUX_PROCESS_OPTIONS","average","percentile",body+";return measureWindow;");
 const measure=make(capture,async()=>{proc++;return snapshot;},()=>({cpuPercent:1,residentBytes:1}),{now:()=>wall+=5},f=>f(),60,{},xs=>xs.reduce((a,b)=>a+b,0)/xs.length,xs=>xs[0]);
 const observer=enabled?{async observe(tree,root,record){observations++;assert.equal(tree,snapshot);assert.equal(record.processes,snapshot);assert.equal(root,100);assert.equal(proc,observations);assert.equal(calls,observations);}}:null;
 const result=await measure({},100,"unrestricted",samples,new ServerCounterClock(),observer);assert.equal(calls,61);assert.equal(proc,61);assert.equal(observations,enabled?61:0);assert.equal(result.completed_frame_delta,2400);assert.equal(samples.length,61);
 }
});

test("first failure survives observation overflow as a valid unavailable numeric report",async()=>{
 const m=await collect({fault:x=>x.index===0&&x.kind==="read"?new Error("first private IO failure"):undefined},62);const r=m.collector.summary();assert.equal(r.observations,62);assert.equal(r.failure_code,2);assert.equal(r.available,0);assert.equal(r.capture_complete,0);assert.equal(r.valid_observations,60);
});

test("actual runner retains after UDP close, before long lifecycle waits, and keeps one failure fallback",async()=>{
 const source=await readFile(new URL("./run-desktop-performance-576-proof.mjs",import.meta.url),"utf8");
 const begin=source.indexOf("    const interaction = await packetProof.interact");
 const end=source.indexOf('    if(proofMode !== "combined")',begin);
 const success=source.slice(begin,end);
 const fallback=source.match(/^  if \(!phaseDiagnosticsRetained\) await retainPhaseDiagnostics\(evidence.sidecarLogPath, phaseWindow\);$/mu)?.[0];assert(fallback);
 const AsyncFunction=Object.getPrototypeOf(async function(){}).constructor;
 const run=new AsyncFunction("context",`const {packetProof,checkpoint,retainPhaseDiagnostics,evidence,phaseWindow,writeFile,path,temporaryRoot,state,requireFixtureSheetActive,reportPath,sheetRecords,executionMode,measurementStartedAt,measurementEndedAt,waitStageComplete,assertRunning,child,finalize}=context;let phaseDiagnosticsRetained=false;try {${success}}catch{}await packetProof.close();await finalize();${fallback}return phaseDiagnosticsRetained;`);
 for(const failure of [null,"interaction","stage"]){let closed=false,bytes=20*1024*1024;const captures=[],events=[],window={startedAt:1000,endedAt:61000};
 const context={packetProof:{async interact(){events.push("interact");if(failure==="interaction")throw Error("injected interaction failure");return {};},async close(){if(!closed)events.push("close");closed=true;},privateState(){return {};}},checkpoint:{result:{proof:{}},progress:{},async save(){},async enter(){}},async retainPhaseDiagnostics(file,actualWindow){assert(closed,"never parse while UDP is open");assert.equal(actualWindow,window);events.push("retain");captures.push({bytes,unavailable:Number(bytes>32*1024*1024)});},evidence:{sidecarLogPath:"private-log"},phaseWindow:window,async writeFile(){},path,temporaryRoot:"private-temp",state:{sheet:{}},async requireFixtureSheetActive(){events.push("sheet");},reportPath:"private-report",sheetRecords:[],executionMode:"unrestricted",measurementStartedAt:1000,measurementEndedAt:61000,async waitStageComplete(){events.push("stage");bytes=40*1024*1024;if(failure==="stage")throw Error("injected Stage failure");return {};},assertRunning(){},child:{},async finalize(){events.push("finalize");bytes=40*1024*1024;}};
 const retained=await run(context);assert.equal(captures.length,1);assert.equal(captures[0].unavailable,Number(failure==="interaction"));assert(events.indexOf("close")<events.indexOf("retain"));
 if(failure==="interaction"){assert.equal(retained,false);assert(events.indexOf("finalize")<events.indexOf("retain"));}else{assert.equal(retained,true);assert(events.indexOf("retain")<events.indexOf("sheet"));assert(events.indexOf("retain")<events.indexOf("stage"));assert(events.indexOf("retain")<events.indexOf("finalize"));}
 }
});
test("locked Tokio family name is recognized without inventing an output caller role",async()=>{
 const lock=await readFile(new URL("../Cargo.lock",import.meta.url),"utf8");assert(lock.includes('name = "tokio"\nversion = "1.53.1"'));
 assert.equal(taskRole({tid:201,comm:"tokio-rt-worker"},200),2);assert.equal(taskRole({tid:201,comm:"tokio-runtime-w"},200),4);
 const report=(await collect()).collector.summary();assert(report.roles[2].cpu_micros>0);assert.equal(report.output_caller_role_available,0);assert.equal(report.cleanup_role_available,0);
});
