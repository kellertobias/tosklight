import test from 'node:test';
import assert from 'node:assert/strict';
import {EventEmitter} from 'node:events';
import {readFile} from 'node:fs/promises';
import crypto from 'node:crypto';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {OwnedPerfBridge,ownedPerfEnabled,parseOwnedIdentity,runOwnedPerfHelper,validateOwnedPerfDiagnostics,validateOwnedPerfEnvelope,validateOwnedPerfHelperReport,validateOwnedPerfProvenance} from './desktop-performance-576-owned-perf.mjs';
const ROOT=fileURLToPath(new URL('../',import.meta.url));
const hash=bytes=>crypto.createHash('sha256').update(bytes).digest('hex');
const candidate='a'.repeat(40),helper=Buffer.from('helper'),paths=Buffer.from('canonical paths');
const pins={schema_version:1,candidate_sha:candidate,helper_sha256:hash(helper),elf_sha256:'b'.repeat(64),elf_build_id:'ab'.repeat(20),perf_sha256:'c'.repeat(64),appimage_sha256:'d'.repeat(64)};
function capture(chains=10){return {samples:10,frames:20,unresolved_self:0,unresolved_ancestor:0,depth_one:0,owned_rust_chains:chains,period:100,elapsed_micros:2000000,recording_envelope_micros:1999999,sample_span_micros:1900000,rows:Array.from({length:14},(_,id)=>({id,self_samples:id===1?10:0,self_period:id===1?100:0,inclusive_samples:id===1?10:0,inclusive_period:id===1?100:0}))};}
function report(){return {schema_version:1,available:1,failure_code:0,failure_stage_code:0,failure_detail_code:0,synthetic_capable:1,rust_unwind_admitted:1,ownership_verified:1,identity_verified:1,thread_scope_verified:1,recorders_stopped:1,raw_private:1,sample_hz:49,requested_seconds:8,preflight_only:0,capture:capture(),preflight:capture(),profile_phase_code:1,exact_measurement_window:0,codegen_flags_changed:0,loss_observed:0,byte_cap_hit:0};}
function envelope(){const {appimage_sha256,...provenance}=pins;return {report:report(),provenance};}
function stat(pid,parent,start){const f=Array(30).fill('0');f[0]='S';f[1]=String(parent);f[19]=String(start);return `${pid} (private name) ${f.join(' ')}\n`;}
const snapshot=[{pid:100,parentPid:500,startTicks:100,command:'desk'},{pid:200,parentPid:100,startTicks:200,command:'light-headless'}];
function fixture(options={}){
 const files=new Map(),writes=[],reads=[],executions=[];let clock=0,sequence=0;
 const config={schema_version:1,candidate_sha:candidate,appimage_sha256:pins.appimage_sha256,elf_sha256:pins.elf_sha256,elf_build_id:pins.elf_build_id,perf_path:'/usr/lib/linux-tools/6.8/perf',perf_sha256:pins.perf_sha256};
 const original={source_commit:candidate,workflow_commit:candidate,source_sha256:{'tools/desktop-performance-576-owned-perf.py':hash(helper),'tools/artifact_paths.py':hash(paths)},binaries:{'Control.AppImage':{sha256:pins.appimage_sha256,bytes:123}}};
 files.set('/out/public/provenance.json',Buffer.from(JSON.stringify(original)));
 files.set('/private/config.json',Buffer.from(JSON.stringify(config)));
 files.set(path.join(ROOT,'tools/desktop-performance-576-owned-perf.py'),helper);
 files.set(path.join(ROOT,'tools/artifact_paths.py'),paths);
 for(const [pid,parent,start] of [[500,1,500],[100,500,100],[200,100,200]])files.set(`/proc/${pid}/stat`,Buffer.from(stat(pid,parent,start)));
 const io={lstat:async file=>{const b=files.get(file);assert(b,`missing ${file}`);return {isFile:()=>true,isSymbolicLink:()=>false,size:file.startsWith('/proc/')?0:b.length};},readFile:async(file,encoding)=>{reads.push(file);assert(files.has(file));const b=files.get(file);return encoding?b.toString():b;},mkdir:async()=>{},writeFile:async(file,bytes,opts)=>{assert.equal(opts.mode,0o600);assert.equal(opts.flag,'wx');assert(!files.has(file));files.set(file,Buffer.from(bytes));writes.push(file);},rename:async(a,b)=>{assert(files.has(a));files.set(b,files.get(a));files.delete(a);},unlink:async file=>{files.delete(file);}};
 const bridge=new OwnedPerfBridge({publicDirectory:'/out/owned-perf-public',requestDirectory:'/private/request',temporaryRoot:'/private/tmp',originalProvenance:'/out/public/provenance.json',environment:{LIGHT_OUTPUT_OWNED_PERF_BINDINGS:'/private/config.json'},runnerPid:500,uid:1001},{io,wait:async ms=>{clock+=ms;},now:()=>clock,uuid:()=>`unique-${++sequence}`,execute:async(...args)=>{executions.push(args);clock+=9000;return {status:0,stdout:JSON.stringify(envelope()),error:false,overflow:false};},...options});
 return {bridge,files,writes,reads,executions,config,original};
}
function publicReport(f){return JSON.parse(f.files.get('/out/owned-perf-public/owned-perf-numeric.json'));}
test('explicit gate enables only full-frequency Control and leaves ordinary modes off',()=>{
 const env={LIGHT_OUTPUT_OWNED_PERF_DIAGNOSTICS:'1',LIGHT_OUTPUT_PHASE_DIAGNOSTICS:'all'};
 assert(ownedPerfEnabled(env,'control-no-stage'));
 for(const mode of ['stage-functional','combined',undefined])assert(!ownedPerfEnabled(env,mode));
 for(const e of [{},{...env,LIGHT_OUTPUT_OWNED_PERF_DIAGNOSTICS:'true'},{...env,LIGHT_OUTPUT_PHASE_DIAGNOSTICS:'true'}])assert(!ownedPerfEnabled(e,'control-no-stage'));
});
test('closed helper reports conserve exclusive bins and keep inclusive bins separate',()=>{
 assert.equal(validateOwnedPerfHelperReport(report()).capture.rows.length,14);
 for(const change of [r=>r.private_path='/secret',r=>r.capture.rows[1].self_samples--,r=>r.capture.rows[1].self_period--,r=>r.capture.rows[1].inclusive_period=101,r=>r.capture.frames=1281,r=>r.capture.sample_span_micros=10000001,r=>r.capture.recording_envelope_micros=2000001,r=>r.capture.rows[0].symbol='raw',r=>r.loss_observed=1,r=>r.failure_stage_code=1,r=>r.available=0]){const r=report();change(r);assert.throws(()=>validateOwnedPerfHelperReport(r));}
});
test('Rust admission matches Python threshold at zero, one, nine and ten chains',()=>{
 for(const chains of [0,1,9,10]){const r=report();r.preflight=capture(chains);r.rust_unwind_admitted=Number(chains>=10);if(chains<10){r.available=0;r.failure_code=13;r.failure_stage_code=10;r.capture=null;}assert.equal(validateOwnedPerfHelperReport(r).rust_unwind_admitted,Number(chains>=10));r.rust_unwind_admitted=1-r.rust_unwind_admitted;assert.throws(()=>validateOwnedPerfHelperReport(r));}
});
test('hash envelope refuses wrong bindings, raw fields and fabricated unbound success',()=>{
 validateOwnedPerfEnvelope(envelope(),pins);validateOwnedPerfProvenance(pins);
 for(const change of [e=>e.provenance.elf_sha256='e'.repeat(64),e=>delete e.provenance.perf_sha256,e=>e.provenance.path='/private',e=>e.report.profile_phase_code=0,e=>e.provenance=null]){const e=envelope();change(e);assert.throws(()=>validateOwnedPerfEnvelope(e,pins));}
 const e=envelope();e.report.available=0;e.report.failure_code=2;e.provenance=null;validateOwnedPerfEnvelope(e,pins);
});
test('stat identity parser uses exact pid and start ticks with bounded malformed rejection',()=>{
 assert.deepEqual(parseOwnedIdentity(stat(200,100,321)),{pid:200,parentPid:100,startTicks:321});
 for(const text of ['','200 (x) S 1',stat(200,100,Number.MAX_SAFE_INTEGER+1),'x'.repeat(8193)])assert.throws(()=>parseOwnedIdentity(text));
});
test('startup writes unavailable numeric report without process probes',async()=>{
 const f=fixture();await f.bridge.initialize();assert.equal(publicReport(f).available,0);assert.equal(publicReport(f).binding_available,1);assert(!f.reads.some(p=>p.startsWith('/proc/')));assert.equal(f.executions.length,0);
 assert.deepEqual(f.files.get('/out/owned-perf-public/provenance.json'),f.files.get('/out/public/provenance.json'));
});
test('missing or mismatched packaged/source pins remain unavailable without invoking helper',async()=>{
 for(const mutate of [f=>f.files.delete('/private/config.json'),f=>{f.config.appimage_sha256='e'.repeat(64);f.files.set('/private/config.json',Buffer.from(JSON.stringify(f.config)));},f=>f.files.set(path.join(ROOT,'tools/artifact_paths.py'),Buffer.from('changed'))]){const f=fixture();mutate(f);await f.bridge.initialize();await f.bridge.capture(100,snapshot);assert.equal(publicReport(f).bridge_failure_code,1);assert.equal(f.executions.length,0);assert(!f.reads.some(p=>p.startsWith('/proc/')));}
});
test('successful capture is once-only after quiet settle, with private exact ownership and packaged pins',async()=>{
 const f=fixture();await f.bridge.initialize();await f.bridge.capture(100,snapshot);await f.bridge.capture(100,snapshot);
 const r=validateOwnedPerfDiagnostics(publicReport(f));assert.equal(r.available,1);assert.equal(r.actual_settle_millis,12000);assert.equal(r.profile_elapsed_millis,9000);assert.equal(r.exact_measurement_window,0);assert.equal(f.executions.length,1);
 const request=JSON.parse(f.files.get('/private/request/owned-perf-request.json'));
 assert.deepEqual([request.runner_pid,request.runner_start_ticks,request.root_pid,request.root_start_ticks,request.target_pid,request.target_start_ticks,request.uid],[500,500,100,100,200,200,1001]);
 assert.equal(request.elf_sha256,pins.elf_sha256);assert.equal(request.perf_sha256,pins.perf_sha256);assert.equal(request.duration_seconds,8);assert.equal(request.allow_root,1);
 assert.equal(path.dirname(f.executions[0][1]),'/private/tmp');assert(!f.files.has(f.executions[0][1]));
 assert.equal([...f.files.keys()].filter(p=>p.startsWith('/out/owned-perf-public/')).length,3);
 assert(!/private name|\/proc\/|runner_pid|target_pid|perf_path|symbol/.test(JSON.stringify(r)));
});
test('PID reuse, root ancestry, missing ownership and short quiet delay fail before execution',async()=>{
 for(const mutate of [f=>f.files.set('/proc/100/stat',Buffer.from(stat(100,500,101))),f=>f.files.set('/proc/200/stat',Buffer.from(stat(200,100,201))),f=>f.files.set('/proc/100/stat',Buffer.from(stat(100,777,100))),f=>{f.bridge.wait=async()=>{};}]){const f=fixture();mutate(f);await f.bridge.initialize();await f.bridge.capture(100,snapshot);assert.equal(publicReport(f).available,0);assert.equal(f.executions.length,0);}
 const f=fixture();await f.bridge.initialize();await f.bridge.capture(100,snapshot.slice(0,1));assert.equal(f.executions.length,0);
});
test('bounded helper failures preserve numeric unavailable without leaking stderr or raw stdout',async()=>{
 for(const result of [{status:137,stdout:'/private/raw',error:false,overflow:false},{status:0,stdout:JSON.stringify({...envelope(),raw:'/secret'}),error:false,overflow:false},{status:0,stdout:JSON.stringify(envelope()),error:false,overflow:true},{status:1,stdout:JSON.stringify(envelope()),error:false,overflow:false}]){const f=fixture({execute:async()=>result});await f.bridge.initialize();await f.bridge.capture(100,snapshot);const r=publicReport(f);assert.equal(r.available,0);assert(r.bridge_failure_code>0);assert(!JSON.stringify(r).includes('/secret'));}
});
test('failed valid Python preflight details remain visible with explicit unavailable state',async()=>{
 const e=envelope();Object.assign(e.report,{available:0,failure_code:13,failure_stage_code:10,rust_unwind_admitted:0,capture:null,preflight:capture(9)});
 const f=fixture({execute:async()=>({status:1,stdout:JSON.stringify(e),error:false,overflow:false})});await f.bridge.initialize();await f.bridge.capture(100,snapshot);const r=publicReport(f);assert.equal(r.helper_report_available,1);assert.equal(r.helper_report.failure_stage_code,10);assert.equal(r.helper_report.preflight.owned_rust_chains,9);assert.equal(r.available,0);
});
test('controlled sudo invokes only explicit paths and canonical overrides, drains bounded output',async()=>{
 const calls=[];const spawnProcess=(command,args,options)=>{calls.push({command,args,options});const child=new EventEmitter();child.stdout=new EventEmitter();child.stderr=new EventEmitter();queueMicrotask(()=>{child.stdout.emit('data',Buffer.from('{}'));child.stderr.emit('data',Buffer.from('/secret raw stacks'));child.emit('close',1);});return child;};
 const r=await runOwnedPerfHelper('/private/request','/private/tmp/owned','/private/tmp',{spawnProcess});assert.equal(r.stdout,'{}');assert.equal(r.status,1);
 assert.equal(calls[0].command,'/usr/bin/sudo');assert.equal(calls[0].options.shell,undefined);assert.deepEqual(calls[0].options.stdio,['ignore','pipe','pipe']);
 const overrides=calls[0].args.filter(x=>x.includes('='));assert(overrides.some(x=>x.startsWith('LIGHT_TMP_DIR=')));assert(overrides.some(x=>x.startsWith('LIGHT_ARTIFACTS_DIR=')));assert(overrides.some(x=>x.startsWith('LIGHT_REPOSITORY_ROOT=')));assert(!overrides.some(x=>/^(HOME|PATH|LD_)/.test(x)));assert(calls[0].args.includes('105s'));
 const huge=await runOwnedPerfHelper('/a','/b','/c',{spawnProcess:()=>{const c=new EventEmitter();c.stdout=new EventEmitter();c.stderr=new EventEmitter();queueMicrotask(()=>{c.stdout.emit('data',Buffer.alloc(65537));c.emit('close',1);});return c;}});assert.equal(huge.overflow,true);assert.equal(huge.stdout,'');
});
test('actual runner invokes profiling after closed UDP and frozen trace, before lifecycle waits',async()=>{
 const source=await readFile(new URL('./run-desktop-performance-576-proof.mjs',import.meta.url),'utf8');
 const block=source.slice(source.indexOf('    const interaction = await packetProof.interact'),source.indexOf('\n\t\tstate.phase = "fixture-sheet-liveness"'));
 const events=[];
 const execute=new Function('packetProof','checkpoint','retainPhaseDiagnostics','evidence','phaseWindow','writeFile','path','temporaryRoot',`return (async()=>{let phaseDiagnosticsRetained=false;const child={pid:100};${block}\n return phaseDiagnosticsRetained;})()`);
 const retained=await execute({interact:async()=>{events.push('fades');return {};},close:async()=>events.push('closed'),privateState:()=>({})},{result:{proof:{}},save:async()=>{},enter:async()=>{}},async()=>events.push('retained'),{sidecarLogPath:'private',samples:[{processes:snapshot}],ownedPerf:{capture:async(root,tree)=>{assert.equal(root,100);assert.deepEqual(tree,snapshot);events.push('profile');}}},{},async()=>{},path,'/private');
 assert(retained);assert.deepEqual(events,['fades','closed','retained','profile']);
});

test('workflow isolates optional profiling and preserves original artifact inventory',async()=>{
 const {createRequire}=await import('node:module');const yaml=createRequire(import.meta.url)('js-yaml');
 const current=yaml.load(await readFile(new URL('../.github/workflows/release-576-proof.yml',import.meta.url),'utf8'));
 const steps=current.jobs['linux-576'].steps;
 for(const prefix of ['Prove default-policy','Verify default-policy']){const step=steps.find(s=>s.name?.startsWith(prefix));assert.equal(step.env.LIGHT_OUTPUT_OWNED_PERF_DIAGNOSTICS,undefined);assert.equal(step.env.LIGHT_OUTPUT_PHASE_DIAGNOSTICS,'true');assert(!step.run.includes('OWNED_PERF'));}
 assert.equal(steps.find(s=>s.name==='Publish closed numeric proof only').with.path.trim().split('\n').length,17);
 const setup=steps.findIndex(s=>s.name==='Prepare separate post-window owned-process profiler'),stage=steps.findIndex(s=>s.name?.startsWith('Verify default-policy software')),trace=steps.findIndex(s=>s.name?.startsWith('Trace default-policy'));assert(stage<setup&&setup<trace);
 assert.equal(steps[setup]['continue-on-error'],true);assert.equal(steps[trace].env.LIGHT_OUTPUT_OWNED_PERF_DIAGNOSTICS,'1');
 assert.equal(steps.filter(s=>s.env?.LIGHT_OUTPUT_OWNED_PERF_DIAGNOSTICS).length,1);
 const upload=steps.find(s=>s.name==='Publish separate numeric owned-process profile');assert.equal(upload.with.path.trim().split('\n').length,3);assert(!upload.with.path.includes('raw'));
 assert(!steps[setup].run.includes('sysctl'));assert(steps[setup].run.includes('assert len(candidates)==1'));assert(steps[setup].run.includes('assert len(perf_candidates)==1'));
});

test('recording and timestamp span preserve the bounded post-window phase',()=>{
 for(const span of [0,2000000]){const r=report();r.capture.sample_span_micros=span;validateOwnedPerfHelperReport(r);}
 const r=report();r.preflight.sample_span_micros=4000001;r.preflight.recording_envelope_micros=5000000;r.preflight.elapsed_micros=6000000;assert.throws(()=>validateOwnedPerfHelperReport(r));
});

test('compiler-only preflight publishes separately before app build without changing original diagnostics',async()=>{
 const {createRequire}=await import('node:module');const yaml=createRequire(import.meta.url)('js-yaml');
 const steps=yaml.load(await readFile(new URL('../.github/workflows/release-576-proof.yml',import.meta.url),'utf8')).jobs['linux-576'].steps;
 const source=steps.findIndex(s=>s.name==='Verify source and observer tests'),preflight=steps.findIndex(s=>s.name==='Diagnose isolated synthetic compiler before application build'),upload=steps.findIndex(s=>s.name==='Publish separate compiler preflight diagnostic before application build'),build=steps.findIndex(s=>s.name==='Build exact canonical release Control and sidecars');
 assert(source<preflight&&preflight<upload&&upload<build);assert.equal(steps[preflight]['continue-on-error'],true);assert.equal(steps[upload]['continue-on-error'],true);
 const script=steps[preflight].run;assert(script.includes('--compiler-preflight --candidate-sha'));assert(script.includes('--kill-after=5s 45s'));assert(script.includes('>"$private/stdout" 2>"$private/stderr"'));assert(!/(?:HOME|PATH|LD_LIBRARY_PATH)=/u.test(script));assert(!script.includes('--request'));assert(!script.includes('--preflight-only'));assert(script.includes('s.size<=65536'));assert(script.includes('!s.isSymbolicLink()'));
 assert.equal(steps[upload].with.path.trim().split('\n').length,2);assert(!steps[upload].with.path.includes('stdout'));assert(!steps[upload].with.path.includes('spool'));
 const start=script.indexOf('          const TOP=')>=0?script.indexOf('          const TOP='):script.indexOf('const TOP='),end=script.indexOf('let report=');
 const validate=new Function('assert',script.slice(start,end)+';return validate;')(assert);
 const row=id=>({id,attempted:1,compiled:0,elf_verified:0,failure_kind:2,tool_failure_code:7,elapsed_micros:10});const valid={schema_version:1,diagnostic_mode_code:1,available:1,source_verified:1,raw_private:1,tools_stopped:1,failure_code:0,attempts:[row(0),{...row(1),compiled:1,elf_verified:1,failure_kind:0,tool_failure_code:0}]};
 assert.equal(validate(valid).available,1);
 for(const mutate of [r=>r.private_path='/secret',r=>r.attempts[0].stderr='private',r=>r.attempts[0].failure_kind=7,r=>r.tools_stopped=0,r=>r.attempts[1].elapsed_micros=-1,r=>r.attempts[1].elapsed_micros=34000001,r=>r.available=0,r=>r.attempts[1].elf_verified=0,r=>r.attempts[1].failure_kind=2,r=>r.attempts[0].failure_kind=0]){const r=structuredClone(valid);mutate(r);assert.throws(()=>validate(r));}
});

test('census reason is fixed bounded numeric data and coherent with failure stage',()=>{
 for(const failure_detail_code of [1,18])for(const failure_stage_code of [7,9,11]){const r=report();Object.assign(r,{available:0,failure_code:11,failure_stage_code,failure_detail_code});validateOwnedPerfHelperReport(r);}
 for(const change of [r=>r.failure_detail_code=19,r=>r.failure_detail_code=-1,r=>r.failure_detail_code=1.5,r=>r.failure_detail_code='header',r=>{r.failure_detail_code=1;},r=>{r.available=0;r.failure_code=7;r.failure_stage_code=7;r.failure_detail_code=1;},r=>{r.available=0;r.failure_code=11;r.failure_stage_code=6;r.failure_detail_code=1;}]){const r=report();change(r);assert.throws(()=>validateOwnedPerfHelperReport(r));}
});
test('bridge preserves unavailable census detail without exposing arbitrary header values',async()=>{
 const e=envelope();Object.assign(e.report,{available:0,failure_code:11,failure_stage_code:7,failure_detail_code:15,rust_unwind_admitted:0,preflight:null,capture:null});
 const f=fixture({execute:async()=>({status:1,stdout:JSON.stringify(e),error:false,overflow:false})});await f.bridge.initialize();await f.bridge.capture(100,snapshot);const r=publicReport(f);assert.equal(r.helper_report_available,1);assert.equal(r.helper_report.failure_code,11);assert.equal(r.helper_report.failure_stage_code,7);assert.equal(r.helper_report.failure_detail_code,15);assert.equal(r.available,0);
});
