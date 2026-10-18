// Pure mocks only: no build, application, process signals or topology subprocess.
import assert from 'node:assert/strict';
import test from 'node:test';
import {createHash} from 'node:crypto';
import {readFile} from 'node:fs/promises';
import {EventEmitter} from 'node:events';
import {pathToFileURL} from 'node:url';
import path from 'node:path';

const ROOT = path.resolve(process.env.LIGHT_REPOSITORY_ROOT ?? process.cwd());
const runnerLocation = new URL("./historical-runner.mjs", import.meta.url);
let runnerSource = await readFile(runnerLocation, "utf8");
runnerSource = runnerSource.replaceAll(
	"import.meta.url",
	JSON.stringify(
		pathToFileURL(path.join(ROOT, "tools/historical-576/historical-runner.mjs"))
			.href,
	),
);
for (const file of [
	"stage-large-scene.mjs",
	"stage-dynamics-scene.mjs",
	"process-tree-resources.mjs",
	"desktop-performance-576-proof-observers.mjs",
	"artifact-paths.mjs",
])
	runnerSource = runnerSource.replace(
		JSON.stringify("../" + file),
		JSON.stringify(pathToFileURL(path.join(ROOT, "tools", file)).href),
	);
for (const file of ["legacy-plan.mjs", "old-benchmark-shim.mjs", "definition-response.mjs", "profile-response.mjs", "owned-process-cleanup.mjs"])
	runnerSource = runnerSource.replace(
		JSON.stringify("./" + file),
		JSON.stringify(new URL("./" + file, import.meta.url).href),
	);

const runnerURL='data:text/javascript;base64,'+Buffer.from(runnerSource).toString('base64');
const {REPORT_FIELDS,PROVENANCE_FIELDS}=await import(runnerURL);
const location=new URL('./run-comparison.mjs',import.meta.url);
let source=await readFile(location,'utf8');
source=source.replaceAll('import.meta.url',JSON.stringify(location.href))
 .replace("'../artifact-paths.mjs'",JSON.stringify(pathToFileURL(path.resolve('tools/artifact-paths.mjs')).href))
 .replace("'./legacy-plan.mjs'",JSON.stringify(new URL('./legacy-plan.mjs',import.meta.url).href))
 .replace("'./historical-runner.mjs'",JSON.stringify(runnerURL));
const m=await import('data:text/javascript;base64,'+Buffer.from(source).toString('base64'));
const OLD='f67c84e48baf96a6b8bc667edca7fb859e852ac5',CURRENT='7a5468514967fca9066c2d00025a3ddb83446d39',H='a'.repeat(64),I='0034aca0703cf144f23ee094f0f5d645eacee08711df7b835ddbb47bae785f3f';
function row(code,available=1){return {...Object.fromEntries(REPORT_FIELDS.map(k=>[k,0])),schema_version:2,setup_checkpoint_code:available?9:0,arm_code:code,available,complete:available,cleanup_verified:1,failure_stage_code:available?0:6,compatibility_build:Number(code===2),fixtures:576,occupied_slots:8064,universes:16,animated_assignments:5408,dynamic_instances:20,counter_observations:available?61:0,interval_count:available?60:0,sheet_available:available,stage_absence_available:available,packet_inventory_available:available,packet_universes:16,static_fades_passed:2,static_fade_commands:2,stage_absence_scope_code:1,sheet_records:576,sheet_heartbeat_count:2,decoded_ring_loss_checked:1,completed_frames:2400,window_lower_millis:60000,window_upper_millis:60001,process_observations:61,runtime_before_checked:1,runtime_before_instances:20,runtime_before_controllers:20,runtime_before_assignments:5408,runtime_after_checked:1,runtime_after_instances:20,runtime_after_controllers:20,runtime_after_assignments:5408};}
function seal(code,available=1){return {...Object.fromEntries(PROVENANCE_FIELDS.filter(k=>k.endsWith('_sha256')).map(k=>[k,H])),schema_version:3,arm_code:code,available,revision_sha:code===1?OLD:CURRENT,input_sha256:I};}
function state(){return {schema_version:1,available:0,failure_stage_code:0,build_completed_arms:3,build_failure_arm_code:0,build_failure_stage_code:0,build_failure_error_code:0,completed_cases:0,same_job:1,logical_cpu_count:4,guest_core_count:2,guest_package_count:1,stage_enabled:0,fixture_sheet_required:1,requested_hz:40,exact_typed_legacy_output_equivalence:0,schema5_acceptance_claim:0,rows:[]};}
const manifest={arms:[1,2,3].map(code=>({revision_sha:code===1?OLD:CURRENT,application_sha256:H,source_manifest_sha256:H,overlay_sha256:H}))};
async function collect(fail=-1,unclean=-1){const result=state(),provenance={cases:[]};let calls=0;const action=m.collectCases({result,provenance,manifest,commonHash:H,runnerHash:H,translatorHash:H,expectedPlans:{canonical:H,selected:[H,H,H]},pause:async()=>{},publish:async()=>m.validateSummary(result),run:async(r,p,a)=>{const n=calls++;const value=row(a+1,Number(n!==fail));if(n===unclean)value.cleanup_verified=0;return {status:value.available?0:1,row:value,seal:seal(a+1)};}});return {action,result,provenance,count:()=>calls};}
test('nine successful rows may remain unavailable until final seals finish',async()=>{const x=await collect();await x.action;assert.equal(x.result.available,1);x.result.available=0;assert.equal(m.validateSummary(x.result),x.result);x.result.available=1;x.result.completed_cases=8;assert.throws(()=>m.validateSummary(x.result));});
test('cleanly failed middle case retains row and all later independent cases',async()=>{const x=await collect(2);await x.action;assert.equal(x.count(),9);assert.equal(x.result.rows.length,9);assert.equal(x.result.completed_cases,8);assert.equal(x.result.available,0);assert.equal(x.result.failure_stage_code,3);});
test('unverified cleanup preserves failed evidence and aborts future launches',async()=>{const x=await collect(1,1);await assert.rejects(x.action);assert.equal(x.count(),2);assert.equal(x.result.rows.length,2);});
test('case seals bind every source/application/runner/translator/common hash',()=>{for(const k of ['revision_sha','application_sha256','source_manifest_sha256','overlay_sha256','common_manifest_sha256','runner_sha256','translator_sha256','input_sha256','canonical_plan_sha256','selected_plan_sha256']){const s=seal(1);s[k]='b'.repeat(k==='revision_sha'?40:64);assert.throws(()=>m.bindCase(s,row(1),manifest,0,H,H,H,{canonical:H,selected:[H,H,H]}),k);}assert.throws(()=>m.bindCase(seal(2),row(1),manifest,0,H,H,H,{canonical:H,selected:[H,H,H]}));});
test('unavailable zero provenance allowed but cannot certify available case',()=>{const s={...seal(1,0),revision_sha:'0'.repeat(40)};assert.doesNotThrow(()=>m.validateCaseProvenance(s));assert.throws(()=>m.bindCase(s,row(1),manifest,0,H,H,H,{canonical:H,selected:[H,H,H]}));});
function build(){return {schema_version:3,old_sha:OLD,current_sha:CURRENT,all_arm_sources_reverified_final:1,helper_sha256:H,packages:Object.fromEntries(m.PACKAGE_FILES.map(name=>[name,H])),inherited_selected_environment:{PATH:'/private/secret',LIGHT_TMP_DIR:'/private/secret'},arms:['old-legacy','current-legacy','current-typed'].map((arm,i)=>({arm,generated_tauri_outputs:Object.fromEntries(m.TAURI_OUTPUT_FILES.map(name=>[name,H])),generated_tauri_outputs_count:4,generated_tauri_outputs_sha256:createHash('sha256').update(JSON.stringify(Object.fromEntries(m.TAURI_OUTPUT_FILES.map(name=>[name,H])))).digest('hex'),postbuild_manifest_sha256:H,source_sha:i===0?OLD:CURRENT,source_files:i===0?6682:8328,effective_manifest_sha256:H,overlay:null,isolated_target:1,appimage:{sha256:H,bytes:10},packaged_elfs:['light-headless','viz-renderer','light-desktop'].map(role=>({role,sha256:H,bytes:10}))}))};}
test('public projection drops all environment/private paths and rejects extra keys',()=>{const b=m.projectBuild(build());assert(!JSON.stringify(b).includes('/private'));assert(!JSON.stringify(b).includes('PATH'));const p={schema_version:1,old_sha:OLD,current_sha:CURRENT,input_sha256:I,orchestrator_sha256:H,build:b,cases:[]};m.validatePublicProvenance(p);assert.throws(()=>m.validatePublicProvenance({...p,path:'/secret'}));b.arms[0].path='/secret';assert.throws(()=>m.validateBuild(b));});
test('build projection refuses incomplete, wrong-arm or malformed ELF receipts',()=>{for(const edit of [b=>b.all_arm_sources_reverified_final=0,b=>b.arms.reverse(),b=>b.arms[0].packaged_elfs.pop(),b=>b.arms[0].appimage.sha256='/secret']){const b=build();edit(b);assert.throws(()=>m.projectBuild(b));}});
test('command waits cleanup on normal nonzero exit and timeout',async()=>{for(const timeout of [false,true]){const child=new EventEmitter();Object.assign(child,{pid:123,exitCode:null,signalCode:null});let closed=false;const result=m.command(['never'],timeout?1:1000,{launch:()=>{if(!timeout)setImmediate(()=>{child.exitCode=1;child.emit('close',1);});return child;},close:async c=>{assert.equal(c,child);await Promise.resolve();closed=true;}});if(timeout)await assert.rejects(result);else assert.equal(await result,1);assert.equal(closed,true);}});
test('command spawn failure still runs bounded cleanup before rejection',async()=>{const child=new EventEmitter();let cleaned=false;const p=m.command(['never'],1000,{launch:()=>{queueMicrotask(()=>child.emit('error',Error('spawn')));return child;},close:async()=>{cleaned=true;}});await assert.rejects(p);assert(cleaned);});
test('group cleanup reaps leader then checks exact owned PGID; no global signals',async()=>{const child=new EventEmitter();Object.assign(child,{pid:123,exitCode:null,signalCode:null});const calls=[];await m.closeOwnedGroup(child,{pause:async()=>{},kill:(pid,signal)=>{calls.push([pid,signal]);assert.equal(pid,-123);if(signal==='SIGTERM'){child.signalCode='SIGTERM';child.emit('close',null);}else if(signal===0){const e=Error('absent');e.code='ESRCH';throw e;}}});assert.deepEqual(calls,[[-123,'SIGTERM'],[-123,0]]);});
test('group closure refusal blocks command completion instead of detached kill timer',async()=>{const child=new EventEmitter();Object.assign(child,{pid:123,exitCode:1,signalCode:null});await assert.rejects(m.closeOwnedGroup(child,{pause:async()=>{},kill:()=>{}}));});

test('fixed package codes and revision source counts cannot drift',()=>{for(const edit of [b=>{b.packages.unknown=H;delete b.packages[m.PACKAGE_FILES[0]];},b=>b.arms[0].source_files=8328]){const b=build();edit(b);assert.throws(()=>m.projectBuild(b));}});

test('build failure exposes only closed bounded numeric status',async()=>{const r=state();await m.captureBuildFailure(r,async()=>({schema_version:1,arm_code:1,stage_code:7,error_code:2}));assert.deepEqual([r.build_failure_arm_code,r.build_failure_stage_code,r.build_failure_error_code],[1,7,2]);m.validateSummary(r);for(const change of [{path:'/secret'},{arm_code:4},{stage_code:15},{error_code:6},{stage_code:0.5},{schema_version:2}]){const x=state();await m.captureBuildFailure(x,async()=>({...{schema_version:1,arm_code:1,stage_code:7,error_code:2},...change}));assert.deepEqual([x.build_failure_arm_code,x.build_failure_stage_code,x.build_failure_error_code],[0,0,0]);}const x=state();await m.captureBuildFailure(x,async()=>{throw Error('/secret');});assert.equal(x.build_failure_error_code,0);});

test('generated Tauri outputs have exact private paths and sealed numeric-only public metadata',()=>{const source=build(), projected=m.projectBuild(source);assert.equal(projected.schema_version,2);for(const a of projected.arms){assert.equal(a.generated_tauri_outputs_count,4);assert.equal(a.postbuild_manifest_sha256,H);assert(!Object.hasOwn(a,'generated_tauri_outputs'));}for(const edit of [b=>delete b.arms[0].generated_tauri_outputs[m.TAURI_OUTPUT_FILES[0]],b=>b.arms[0].generated_tauri_outputs['secret/path']=H,b=>b.arms[0].generated_tauri_outputs_count=3,b=>b.arms[0].generated_tauri_outputs_sha256='0'.repeat(64),b=>b.arms[0].postbuild_manifest_sha256='/private']){const b=build();edit(b);assert.throws(()=>m.projectBuild(b));}});
