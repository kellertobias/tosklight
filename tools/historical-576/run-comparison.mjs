// CI-only three-arm orchestration. Private build environments never enter public receipts.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {readFile,writeFile,mkdir,lstat} from 'node:fs/promises';
import {spawn,execFileSync} from 'node:child_process';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {artifactPaths} from '../artifact-paths.mjs';
import {translateLegacyPlan} from './legacy-plan.mjs';
import {ARMS,validateRow,validateProvenance,provenancePath,validateManifest,stable} from './historical-runner.mjs';
const ROOT=fileURLToPath(new URL('../../',import.meta.url));
const OLD='f67c84e48baf96a6b8bc667edca7fb859e852ac5',CURRENT='7a5468514967fca9066c2d00025a3ddb83446d39';
const INPUT='0034aca0703cf144f23ee094f0f5d645eacee08711df7b835ddbb47bae785f3f';
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const closed=(row,keys)=>assert.deepEqual(Object.keys(row).sort(),keys.slice().sort());
const hex=(value,length)=>typeof value==='string'&&new RegExp(`^[a-f0-9]{${length}}$`,'u').test(value);
const uint=value=>Number.isSafeInteger(value)&&value>=0;
const sleep=ms=>new Promise(resolve=>setTimeout(resolve,ms));
export const ORDERS=Object.freeze([[0,1,2],[2,1,0],[0,1,2]].map(Object.freeze));
export const SUMMARY_FIELDS=Object.freeze(['schema_version','available','failure_stage_code','build_completed_arms','build_failure_arm_code','build_failure_stage_code','build_failure_error_code','completed_cases','same_job','logical_cpu_count','guest_core_count','guest_package_count','stage_enabled','fixture_sheet_required','requested_hz','exact_typed_legacy_output_equivalence','schema5_acceptance_claim','rows']);
export function validateSummary(row){
  closed(row,SUMMARY_FIELDS);assert.equal(row.schema_version,1);assert([0,1].includes(row.available));
  for(const key of SUMMARY_FIELDS.filter(k=>k!=='rows'))assert(uint(row[key]));
  assert(row.build_failure_arm_code<=3&&row.build_failure_stage_code<=14&&row.build_failure_error_code<=5);
  if(row.available)assert.equal(row.build_failure_arm_code+row.build_failure_stage_code+row.build_failure_error_code,0);
  assert(row.failure_stage_code<=5&&row.build_completed_arms<=3&&row.completed_cases<=9);
  assert.equal(row.same_job,1);
  for(const [key,wanted] of [['logical_cpu_count',4],['guest_core_count',2],['guest_package_count',1]])assert(row[key]===0||row[key]===wanted);
  for(const [key,wanted] of [['stage_enabled',0],['fixture_sheet_required',1],['requested_hz',40],['exact_typed_legacy_output_equivalence',0],['schema5_acceptance_claim',0]])assert.equal(row[key],wanted);
  assert(Array.isArray(row.rows)&&row.rows.length<=9);
  row.rows.forEach((entry,index)=>{closed(entry,['round','position','result']);assert.equal(entry.round,Math.floor(index/3));assert.equal(entry.position,index%3);validateRow(entry.result);assert.equal(entry.result.arm_code,ORDERS[entry.round][entry.position]+1);});
  assert.equal(row.completed_cases,row.rows.filter(x=>x.result.available).length);
  if(row.available){assert.equal(row.build_completed_arms,3);assert.equal(row.completed_cases,9);assert.equal(row.failure_stage_code,0);assert.equal(row.logical_cpu_count,4);assert.equal(row.guest_core_count,2);assert.equal(row.guest_package_count,1);}return row;
}
const ROLES=['light-headless','viz-renderer','light-desktop'];
export const PACKAGE_FILES=Object.freeze(['robe--robin-dls-profile','robe--robin-600x-ledwash','showtec--sunstrip-led-rgb-42206','robe--robin-ledbeam-150','generic--dimmer','venue--stage-element-2-1-m','venue--stage-stairs','venue--four-point-truss','venue--curtain-5-m'].map(n=>'assets/fixture-library/'+n+'.toskfixture').sort());
export const TAURI_OUTPUT_FILES=Object.freeze(['acl-manifests.json','capabilities.json','desktop-schema.json','linux-schema.json'].map(name=>'apps/light-desktop/src-tauri/gen/schemas/'+name));
export function projectBuild(build){
  assert.equal(build.schema_version,3);assert.equal(build.old_sha,OLD);assert.equal(build.current_sha,CURRENT);assert.equal(build.all_arm_sources_reverified_final,1);assert.equal(build.arms.length,3);
  const packages=Object.entries(build.packages).sort(([a],[b])=>a<b?-1:a>b?1:0);assert.deepEqual(packages.map(([name])=>name),PACKAGE_FILES);
  return validateBuild({schema_version:2,helper_sha256:build.helper_sha256,packages:packages.map(([,sha256],index)=>({package_code:index+1,sha256})),arms:build.arms.map((a,index)=>{
    assert.equal(a.arm,ARMS[index]);assert.equal(a.source_sha,index===0?OLD:CURRENT);assert.equal(a.isolated_target,1);
    closed(a.generated_tauri_outputs,TAURI_OUTPUT_FILES);for(const value of Object.values(a.generated_tauri_outputs))assert(hex(value,64));assert.equal(a.generated_tauri_outputs_count,4);assert.equal(a.generated_tauri_outputs_sha256,hash(stable(a.generated_tauri_outputs)));assert(hex(a.postbuild_manifest_sha256,64));
    return {generated_tauri_outputs_count:a.generated_tauri_outputs_count,generated_tauri_outputs_sha256:a.generated_tauri_outputs_sha256,postbuild_manifest_sha256:a.postbuild_manifest_sha256,arm_code:index+1,source_sha:a.source_sha,source_files:a.source_files,effective_manifest_sha256:a.effective_manifest_sha256,overlay_sha256:hash(stable(a.overlay)),application_sha256:a.appimage.sha256,application_bytes:a.appimage.bytes,packaged_elfs:a.packaged_elfs.map((elf,i)=>{assert.equal(elf.role,ROLES[i]);return {role_code:i+1,sha256:elf.sha256,bytes:elf.bytes};})};
  })});
}
export function validateBuild(build){
  closed(build,['schema_version','helper_sha256','packages','arms']);assert.equal(build.schema_version,2);assert(hex(build.helper_sha256,64));assert.equal(build.packages.length,9);assert.equal(build.arms.length,3);
  build.packages.forEach((p,i)=>{closed(p,['package_code','sha256']);assert.equal(p.package_code,i+1);assert(hex(p.sha256,64));});
  build.arms.forEach((a,i)=>{closed(a,['generated_tauri_outputs_count','generated_tauri_outputs_sha256','postbuild_manifest_sha256','arm_code','source_sha','source_files','effective_manifest_sha256','overlay_sha256','application_sha256','application_bytes','packaged_elfs']);assert.equal(a.arm_code,i+1);assert.equal(a.source_sha,i===0?OLD:CURRENT);assert.equal(a.source_files,i===0?6682:8328);assert(uint(a.application_bytes)&&a.application_bytes>0);assert.equal(a.generated_tauri_outputs_count,4);for(const k of ['generated_tauri_outputs_sha256','postbuild_manifest_sha256','effective_manifest_sha256','overlay_sha256','application_sha256'])assert(hex(a[k],64));assert.equal(a.packaged_elfs.length,3);a.packaged_elfs.forEach((e,j)=>{closed(e,['role_code','sha256','bytes']);assert.equal(e.role_code,j+1);assert(hex(e.sha256,64)&&uint(e.bytes)&&e.bytes>0);});});return build;
}
export function validateCaseProvenance(row){validateProvenance(row);assert(row.revision_sha===(row.arm_code===1?OLD:CURRENT)||(!row.available&&row.revision_sha==='0'.repeat(40)));if(row.available)assert.equal(row.input_sha256,INPUT);return row;}
export function validatePublicProvenance(row){
  closed(row,['schema_version','old_sha','current_sha','input_sha256','orchestrator_sha256','build','cases']);assert.equal(row.schema_version,1);assert.equal(row.old_sha,OLD);assert.equal(row.current_sha,CURRENT);assert.equal(row.input_sha256,INPUT);assert(hex(row.orchestrator_sha256,64));if(row.build!==null)validateBuild(row.build);assert(Array.isArray(row.cases)&&row.cases.length<=9);row.cases.forEach(validateCaseProvenance);return row;
}
export function bindCase(seal,row,manifest,armIndex,commonHash,runnerHash,translatorHash,expectedPlans){
  validateCaseProvenance(seal);assert.equal(seal.arm_code,armIndex+1);const arm=manifest.arms[armIndex];
  if(seal.available){for(const key of ['revision_sha','application_sha256','source_manifest_sha256','overlay_sha256'])assert.equal(seal[key],arm[key]);assert.equal(seal.common_manifest_sha256,commonHash);assert.equal(seal.input_sha256,INPUT);assert.equal(seal.runner_sha256,runnerHash);assert.equal(seal.translator_sha256,translatorHash);assert.equal(seal.canonical_plan_sha256,expectedPlans.canonical);assert.equal(seal.selected_plan_sha256,expectedPlans.selected[armIndex]);}
  if(row.available)assert.equal(seal.available,1);return seal;
}
export function validateBuildStatus(row){closed(row,['schema_version','arm_code','stage_code','error_code']);assert.equal(row.schema_version,1);for(const key of ['arm_code','stage_code','error_code'])assert(uint(row[key]));assert(row.arm_code<=3&&row.stage_code<=14&&row.error_code<=5);return row;}
export async function captureBuildFailure(result,read){try{const status=validateBuildStatus(await read());result.build_failure_arm_code=status.arm_code;result.build_failure_stage_code=status.stage_code;result.build_failure_error_code=status.error_code;}catch{/* Missing or invalid private status conveys no diagnostics. */}}
async function bounded(file,cap){const stat=await lstat(file);assert(stat.isFile()&&!stat.isSymbolicLink()&&stat.size>0&&stat.size<=cap);const bytes=await readFile(file);assert(bytes.length<=cap);return bytes;}
async function json(file,cap=4*1024*1024){return JSON.parse(await bounded(file,cap));}
async function save(file,value){await writeFile(file,JSON.stringify(value)+'\n',{mode:0o600});}
export async function closeOwnedGroup(child,{kill=process.kill.bind(process),pause=sleep}={}){
  if(!uint(child.pid)||child.pid===0)return;
  const exited=new Promise(resolve=>{if(child.exitCode!==null||child.signalCode!==null)resolve();else child.once('close',resolve);});
  for(const signal of ['SIGTERM','SIGKILL']){
    try{kill(-child.pid,signal);}catch(e){if(e.code!=='ESRCH')throw e;}
    await Promise.race([exited,pause(3000)]);
    for(let i=0;i<30;i++){try{kill(-child.pid,0);}catch(e){if(e.code==='ESRCH'){await Promise.race([exited,pause(3000)]);assert(child.exitCode!==null||child.signalCode!==null);return;}throw e;}await pause(100);}
  }throw Error('owned group closure unavailable');
}
export async function command(args,timeout,{launch=spawn,close=closeOwnedGroup}={}){
  const child=launch(args[0],args.slice(1),{cwd:ROOT,stdio:'ignore',detached:true});let timer;
  try{return await new Promise((resolve,reject)=>{timer=setTimeout(()=>reject(Error('comparison command deadline')),timeout);child.once('error',reject);child.once('close',code=>resolve(code));});}
  finally{clearTimeout(timer);await close(child);}
}
export function topology(){
  const bytes=execFileSync('python3',['-c',"import os,pathlib,json\na=sorted(os.sched_getaffinity(0));t=[]\nassert 0<len(a)<=4096\nfor c in a:\n p=pathlib.Path(f'/sys/devices/system/cpu/cpu{c}/topology');v=tuple(int((p/n).read_text()) for n in ['physical_package_id','core_id']);assert min(v)>=0;t.append(v)\nprint(json.dumps({'logical_cpu_count':len(a),'guest_core_count':len(set(t)),'guest_package_count':len({x[0] for x in t})}))"],{timeout:10000,maxBuffer:4096,stdio:['ignore','pipe','ignore']});
  const value=JSON.parse(bytes);closed(value,['logical_cpu_count','guest_core_count','guest_package_count']);assert.equal(value.logical_cpu_count,4);assert.equal(value.guest_core_count,2);assert.equal(value.guest_package_count,1);return value;
}
function manifestFromBuild(build,input){return {schema_version:1,input,arms:build.arms.map(a=>({id:ARMS[a.arm_code-1],revision_sha:a.source_sha,application_sha256:a.application_sha256,source_manifest_sha256:a.effective_manifest_sha256,overlay_sha256:a.overlay_sha256,compatibility_build:Number(a.arm_code===2)}))};}
export async function collectCases({result,provenance,manifest,commonHash,runnerHash,translatorHash,expectedPlans,run,publish,pause=sleep}){
  for(let round=0;round<3;round++)for(let position=0;position<3;position++){
    const armIndex=ORDERS[round][position];const {status,row,seal}=await run(round,position,armIndex);
    assert(status===0||status===1);validateRow(row);assert.equal(status,Number(!row.available));assert.equal(row.arm_code,armIndex+1);
    // Unknown/missing receipts and unverified cleanup abort future launches.
    bindCase(seal,row,manifest,armIndex,commonHash,runnerHash,translatorHash,expectedPlans);
    result.rows.push({round,position,result:row});result.completed_cases+=row.available;provenance.cases.push(seal);await publish();assert.equal(row.cleanup_verified,1);await pause(5000);
  }
  const valid=provenance.cases.filter(x=>x.available);if(valid.length!==9||result.completed_cases!==9){result.failure_stage_code=3;return;}
  assert.equal(new Set(valid.map(x=>x.canonical_plan_sha256)).size,1);assert.equal(new Set(valid.filter(x=>x.arm_code!==3).map(x=>x.selected_plan_sha256)).size,1);assert.equal(new Set(valid.map(x=>x.common_manifest_sha256)).size,1);result.available=1;
}
export async function runComparison(){
  assert.equal(process.platform,'linux');assert(!Object.hasOwn(process.env,'LIGHT_OUTPUT_WORKERS'));
  const suffix=`${process.env.GITHUB_RUN_ID}-${process.env.GITHUB_RUN_ATTEMPT}`;assert(/^\d+-\d+$/u.test(suffix));
  const publicRoot=path.join(artifactPaths.results,`historical-576-${suffix}`),privateRoot=path.join(artifactPaths.tmp,`historical-576-${suffix}`);await mkdir(publicRoot,{recursive:true,mode:0o700});
  const result={schema_version:1,available:0,failure_stage_code:0,build_completed_arms:0,build_failure_arm_code:0,build_failure_stage_code:0,build_failure_error_code:0,completed_cases:0,same_job:1,logical_cpu_count:0,guest_core_count:0,guest_package_count:0,stage_enabled:0,fixture_sheet_required:1,requested_hz:40,exact_typed_legacy_output_equivalence:0,schema5_acceptance_claim:0,rows:[]};
  const provenance={schema_version:1,old_sha:OLD,current_sha:CURRENT,input_sha256:INPUT,orchestrator_sha256:hash(await readFile(fileURLToPath(import.meta.url))),build:null,cases:[]};
  const publish=async()=>{validateSummary(result);validatePublicProvenance(provenance);await save(path.join(publicRoot,'comparison-numeric.json'),result);await save(path.join(publicRoot,'comparison-provenance.json'),provenance);};
  await publish();let stage=1;
  try{
    Object.assign(result,topology());const inputPath=process.env.LIGHT_HISTORICAL_INPUT;assert(inputPath&&path.isAbsolute(inputPath));const inputBytes=await bounded(inputPath,32*1024*1024);assert.equal(hash(inputBytes),INPUT);const input=JSON.parse(inputBytes),canonical=hash(stable(input.plan)),legacy=hash(stable(translateLegacyPlan(input.plan))),expectedPlans={canonical,selected:[legacy,legacy,canonical]};
    stage=2;try{assert.equal(await command(['python3','-B',path.join(ROOT,'tools/historical-576/build-comparison.py'),privateRoot],7_300_000),0);}catch(error){await captureBuildFailure(result,()=>json(path.join(privateRoot,'build-status-private.json'),4096));throw error;}
    const build=projectBuild(await json(path.join(privateRoot,'build-provenance.json')));provenance.build=build;result.build_completed_arms=3;
    const manifest=manifestFromBuild(build,{path:inputPath,sha256:INPUT});for(const arm of ARMS)validateManifest(manifest,arm);const manifestFile=path.join(privateRoot,'comparison-manifest.json');await save(manifestFile,manifest);
    const commonHash=hash(JSON.stringify(manifest)+'\n'),runnerHash=hash(await readFile(path.join(ROOT,'tools/historical-576/historical-runner.mjs'))),translatorHash=hash(await readFile(path.join(ROOT,'tools/historical-576/legacy-plan.mjs')));stage=3;
    await collectCases({result,provenance,manifest,commonHash,runnerHash,translatorHash,expectedPlans,publish,run:async(round,position,armIndex)=>{
      const name=`round-${round}-position-${position}`,numeric=path.join(privateRoot,name+'.json');const status=await command(['xvfb-run','-a','node','tools/historical-576/historical-runner.mjs','--arm',ARMS[armIndex],'--application',path.join(privateRoot,ARMS[armIndex],'Control.AppImage'),'--output-dir',path.join(privateRoot,name),'--numeric-report',numeric,'--manifest',manifestFile],450_000);
      return {status,row:await json(numeric),seal:await json(provenancePath(numeric))};
    }});stage=4;await publish();return result;
  }catch{result.failure_stage_code=stage;result.available=0;await publish();return result;}
}
if(process.argv[1]&&path.resolve(process.argv[1])===fileURLToPath(import.meta.url)){const result=await runComparison();console.log(JSON.stringify({available:result.available,completed_cases:result.completed_cases,failure_stage_code:result.failure_stage_code}));process.exitCode=result.available?0:1;}
