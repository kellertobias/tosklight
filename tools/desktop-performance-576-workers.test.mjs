import test from "node:test";
import assert from "node:assert/strict";
import {readFile,mkdtemp,rm,stat,readdir} from "node:fs/promises";
import path from "node:path";
import {artifactPaths} from "./artifact-paths.mjs";
import {WORKER_LIMITS,expectedWorkerReceiptCount,bindOwnedHeadless,parseOutputWorkerNames,captureOwnedWorkerThreads,WorkerCountReceipt,validateWorkerDiagnostics,writeWorkerDiagnostics} from "./desktop-performance-576-workers.mjs";
const root={pid:100,parentPid:1,startTicks:123,command:"light-desktop"},head={pid:200,parentPid:100,startTicks:456,command:"light-headless"},snapshot=[root,head];
function proc(p){const fields=Array(22).fill("0");fields[0]="S";fields[1]=String(p.parentPid);fields[19]=String(p.startTicks);return `${p.pid} (${p.command}) ${fields.join(" ")}\n`;}
function mock({names=["light-headless\n","light-output-0\n","light-output-1\n"],change=null,throwRead=false,cap=false}={}){
 let listCalls=0,statCalls=0;const calls=[];
 const io={async readdir(file){calls.push(file);listCalls++;return cap?Array.from({length:4097},(_,i)=>String(i+1)):names.map((_,i)=>String(201+i)).concat(change&&listCalls%2===0?["999"]:[]).reverse();},async readFile(file){calls.push(file);if(throwRead)throw new Error("unavailable /private/path");if(file===`/proc/${root.pid}/stat`)return proc(root);if(file===`/proc/${head.pid}/stat`){statCalls++;return proc({...head,startTicks:change==="pid"&&statCalls>1?457:456});}const match=/\/task\/(\d+)\/comm$/u.exec(file);assert(match);return names[Number(match[1])-201];}};
 return {io,calls};
}
function clock(values){let index=0;return ()=>values?.[index++]??index;}
async function receipt(expected=2,options={}){const fixture=mock(options),r=new WorkerCountReceipt(expected,{io:fixture.io,now:clock()});await r.observe(snapshot,100);await r.observe(snapshot,100);return r;}
test("owned snapshot binding follows ancestry and rejects missing, ambiguous, unsafe or cyclic identities",()=>{
 assert.deepEqual(bindOwnedHeadless(snapshot,100),{pid:200,startTicks:456,rootPid:100,rootStartTicks:123});
 assert.deepEqual(bindOwnedHeadless([...snapshot,{pid:201,parentPid:999,startTicks:1,command:"light-headless"}],100),bindOwnedHeadless(snapshot,100));
 for(const rows of [[],[head],[root],snapshot.concat({...head,pid:201}),snapshot.concat(head),[root,{...head,parentPid:200}],[root,{...head,startTicks:Number.MAX_SAFE_INTEGER+1}]])assert.throws(()=>bindOwnedHeadless(rows,100));
});
test("one worker proves absent output pool threads; two to four prove complete index sets including multiple pools",()=>{
 assert.deepEqual(parseOutputWorkerNames(["light-headless\n","tokio-runtime-w\n"],1),{task_count:2,output_thread_count:0,pool_count:0,index_counts:[0,0,0,0],matches_expected:1});
 for(let workers=2;workers<=4;workers++)for(const pools of [1,2]){const names=["light-headless\n",...Array.from({length:pools},()=>Array.from({length:workers},(_,i)=>`light-output-${i}\n`)).flat()];const r=parseOutputWorkerNames(names,workers);assert.equal(r.matches_expected,1);assert.equal(r.pool_count,pools);assert.deepEqual(r.index_counts,Array.from({length:4},(_,i)=>i<workers?pools:0));}
 for(const [names,expected]of [[["light-output-0\n"],1],[["light-output-0\n"],2],[["light-output-0\n","light-output-0\n","light-output-1\n"],2],[["light-output-0\n","light-output-1\n","light-output-2\n"],2]])assert.equal(parseOutputWorkerNames(names,expected).matches_expected,0);
});
test("malformed task names and invalid task inventory fail closed",()=>{
 for(const name of ["light-output-4\n","light-output-x\n","light-output-00\n","light-output-0","light-output-0\n\n","\n","bad\0name\n","too-long-thread-name\n"])assert.throws(()=>parseOutputWorkerNames([name],2));
 assert.throws(()=>parseOutputWorkerNames([],2));assert.throws(()=>parseOutputWorkerNames(Array(4097).fill("a\n"),2));
});
test("live capture rechecks both process identities and a stable bounded task set",async()=>{
 const fixture=mock(),binding=bindOwnedHeadless(snapshot,100);const actual=await captureOwnedWorkerThreads(binding,fixture.io);assert.equal(actual.names.length,3);assert.deepEqual(actual.ids,["201","202","203"]);assert.equal(fixture.calls.filter(p=>p.endsWith("/stat")).length,4);
 for(const options of [{change:"pid"},{change:"tasks"},{cap:true},{throwRead:true}]){const {io}=mock(options);await assert.rejects(()=>captureOwnedWorkerThreads(binding,io));}
});
test("two successful snapshots produce closed numeric receipts with no raw process identity",async()=>{
 for(const workers of [1,2,3,4]){const names=["light-headless\n",...Array.from({length:workers===1?0:workers},(_,i)=>`light-output-${i}\n`)];const r=await receipt(workers,{names}),report=r.summary();assert.equal(report.available,1);assert.equal(report.capture_complete,1);assert.equal(report.valid_observations,2);assert.deepEqual(validateWorkerDiagnostics(JSON.parse(JSON.stringify(report))),report);const json=JSON.stringify(report);for(const secret of ["pid","startTicks","light-headless","light-output","/proc/","/private/"])assert(!json.includes(secret));assert.equal(r.privateState()[0].binding.pid,200);}
});
test("read failure, PID reuse and thread lifecycle changes retain unavailable diagnostic status",async()=>{
 for(const [options,code]of [[{throwRead:true},2],[{change:"pid"},3],[{cap:true},4],[{change:"tasks"},5],[{names:["light-output-x\n"]},6]]){const r=await receipt(2,options),report=r.summary();assert.equal(report.available,0);assert.equal(report.failure_code,code);assert.equal(report.observations,2);assert.equal(report.capture_complete,0);assert(!JSON.stringify(report).includes("/private/"));}
 const missing=new WorkerCountReceipt(1,{io:mock().io});await missing.observe([root],100);await missing.observe([root],100);assert.equal(missing.summary().failure_code,1);assert.equal(missing.summary().available,0);
 const wrong=await receipt(1);assert.equal(wrong.summary().failure_code,7);assert.equal(wrong.summary().capture_complete,1);assert.equal(wrong.summary().samples[0].output_thread_count,2);
});
test("headless replacement and reordered observations cannot certify stable worker policy",async()=>{
 const r=new WorkerCountReceipt(2,{io:mock().io,now:clock()});await r.observe(snapshot,100);await r.observe([root,{...head,startTicks:457}],100);assert.equal(r.summary().failure_code,3);
 const backwards=new WorkerCountReceipt(2,{io:mock().io,now:clock([10,20,19,30])});await backwards.observe(snapshot,100);await backwards.observe(snapshot,100);assert.equal(backwards.summary().failure_code,8);
 const complete=await receipt();for(let i=0;i<10;i++)await complete.observe(snapshot,100);assert.equal(complete.summary().observations,3);assert.equal(complete.summary().available,0);assert.equal(complete.privateState().length,2);
});
test("forged closed numeric receipts cannot claim capture, geometry or successful unavailable samples",async()=>{
 const good=(await receipt()).summary();for(const change of [r=>r.pid=100,r=>r.expected_workers=3,r=>r.available=0,r=>r.samples[0].index_counts=[1,0,0,0],r=>r.samples[0].pool_count=2,r=>r.samples[0].task_count=0,r=>r.samples[0].failure_code=2,r=>r.valid_observations=1,r=>r.observations=1]){const bad=structuredClone(good);change(bad);assert.throws(()=>validateWorkerDiagnostics(bad));}
});
test("publication is atomic private-mode JSON with no temporary siblings",async()=>{
 const dir=await mkdtemp(path.join(artifactPaths.tmp,"tl775-worker-receipt-"));try{const destination=path.join(dir,"worker-numeric.json"),report=(await receipt()).summary();await writeWorkerDiagnostics(destination,report);assert.equal((await stat(destination)).mode&0o777,0o600);assert.deepEqual(JSON.parse(await readFile(destination,"utf8")),report);assert.deepEqual(await readdir(dir),["worker-numeric.json"]);}finally{await rm(dir,{recursive:true,force:true});}
});


test("proof-only expected workers observes defaults without changing runtime configuration",()=>{
 for(const workers of [1,2,3,4]) {
  const onlyProof=Object.freeze({LIGHT_OUTPUT_WORKER_PROOF_EXPECTED:String(workers)});
  assert.equal(expectedWorkerReceiptCount(onlyProof),workers);assert(!Object.hasOwn(onlyProof,"LIGHT_OUTPUT_WORKERS"));
  const both=Object.freeze({LIGHT_OUTPUT_WORKER_PROOF_EXPECTED:String(workers),LIGHT_OUTPUT_WORKERS:"4"});
  assert.equal(expectedWorkerReceiptCount(both),workers);assert.equal(both.LIGHT_OUTPUT_WORKERS,"4");
  assert.equal(expectedWorkerReceiptCount({LIGHT_OUTPUT_WORKERS:String(workers)}),workers);
 }
 assert.equal(expectedWorkerReceiptCount({}),null);assert.equal(expectedWorkerReceiptCount({LIGHT_OUTPUT_WORKERS:"max"}),null);
 for(const value of [undefined,null,"","0","5","02","2 ","max",2])assert.throws(()=>expectedWorkerReceiptCount({LIGHT_OUTPUT_WORKER_PROOF_EXPECTED:value,LIGHT_OUTPUT_WORKERS:"2"}),/proof-only/);
});
test("proof expectation validation precedes application launch and runtime environment stays inherited",async()=>{
 const source=await readFile(new URL("./run-desktop-performance-576-proof.mjs",import.meta.url),"utf8");
 const run=source.slice(source.indexOf("async function runCase("));
 assert(run.indexOf("expectedWorkerReceiptCount(process.env)")<run.indexOf("spawn(invocation.command"));
 assert(run.includes("...process.env,"));assert(!/LIGHT_OUTPUT_WORKERS\s*[:=]/u.test(run));
});
