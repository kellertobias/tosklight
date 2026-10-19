// Explicit post-window profiling bridge. This never observes the fixed counter interval.
import assert from "node:assert/strict";
import * as fs from "node:fs/promises";
import {spawn} from "node:child_process";
import crypto from "node:crypto";
import path from "node:path";
import {fileURLToPath} from "node:url";
import {artifactRoot} from "./artifact-paths.mjs";
import {bindOwnedHeadless} from "./desktop-performance-576-workers.mjs";
const ROOT=fileURLToPath(new URL("../",import.meta.url));
const HELPER=path.join(ROOT,"tools/desktop-performance-576-owned-perf.py");
const SETTLE=12000,DURATION=8,LIMIT=64*1024;
const uint=n=>Number.isSafeInteger(n)&&n>=0,bit=b=>Number(Boolean(b));
const closed=(value,keys)=>assert.deepEqual(Object.keys(value).sort(),keys.slice().sort());
const hex=(value,length)=>typeof value==="string"&&new RegExp(`^[a-f0-9]{${length}}$`,"u").test(value);
const sha=bytes=>crypto.createHash("sha256").update(bytes).digest("hex");
const sleep=ms=>new Promise(resolve=>setTimeout(resolve,ms));
export function ownedPerfEnabled(env,mode){return env.LIGHT_OUTPUT_OWNED_PERF_DIAGNOSTICS==="1"&&env.LIGHT_OUTPUT_PHASE_DIAGNOSTICS==="all"&&mode==="control-no-stage";}
function safePath(value){assert(typeof value==="string"&&path.isAbsolute(value)&&!/[\r\n\0]/u.test(value));return value;}
async function load(io,file,limit){const s=await io.lstat(file);assert(s.isFile()&&!s.isSymbolicLink()&&s.size>0&&s.size<=limit);const bytes=await io.readFile(file);assert(bytes.length<=limit);return bytes;}
export function parseOwnedIdentity(text){assert(typeof text==="string"&&Buffer.byteLength(text)<=8192);const m=/^([1-9]\d*) \([^\r\n\0]*\) (.*)$/u.exec(text.trim());assert(m);const fields=m[2].split(/\s+/u);assert(fields.length>=20);const pid=Number(m[1]),parentPid=Number(fields[1]),startTicks=Number(fields[19]);assert([pid,parentPid,startTicks].every(uint)&&pid>0);return {pid,parentPid,startTicks};}
const HELPER_KEYS=["schema_version","available","failure_code","failure_stage_code","failure_detail_code","synthetic_capable","rust_unwind_admitted","ownership_verified","identity_verified","thread_scope_verified","recorders_stopped","raw_private","sample_hz","requested_seconds","preflight_only","capture","preflight","profile_phase_code","exact_measurement_window","codegen_flags_changed","loss_observed","byte_cap_hit"];
const CAPTURE_KEYS=["samples","frames","unresolved_self","unresolved_ancestor","depth_one","owned_rust_chains","mapped_owned_resolved_chains","known_rust_plus_other_owned_chains","max_frame_depth","max_mapped_owned_resolved_depth","period","rows","elapsed_micros","recording_envelope_micros","sample_span_micros"];
const BIN_KEYS=["id","self_samples","self_period","inclusive_samples","inclusive_period"];
export function validateOwnedPerfHelperReport(r){
 closed(r,HELPER_KEYS);for(const key of HELPER_KEYS.filter(k=>!["capture","preflight"].includes(k)))assert(uint(r[key]));
 assert(r.schema_version===2&&r.sample_hz===49&&r.failure_code<=17&&r.failure_stage_code<=14&&r.failure_detail_code<=56&&r.requested_seconds>=1&&r.requested_seconds<=10&&r.profile_phase_code===1&&r.exact_measurement_window===0&&r.codegen_flags_changed===0);
 assert(!r.failure_detail_code||(r.failure_code===11&&[7,9,11].includes(r.failure_stage_code)));
 for(const key of ["available","synthetic_capable","rust_unwind_admitted","ownership_verified","identity_verified","thread_scope_verified","recorders_stopped","raw_private","preflight_only","loss_observed","byte_cap_hit"])assert(r[key]<=1);
 for(const key of ["capture","preflight"]){const c=r[key];if(c===null)continue;closed(c,CAPTURE_KEYS);for(const name of CAPTURE_KEYS.filter(k=>k!=="rows"))assert(uint(c[name]));assert(c.samples>0&&c.samples<=10000&&c.frames>=c.samples&&c.frames<=c.samples*128&&c.unresolved_self<=c.samples&&c.unresolved_ancestor<=c.frames-c.samples&&c.depth_one<=c.samples&&c.owned_rust_chains<=c.samples);assert(c.owned_rust_chains<=c.mapped_owned_resolved_chains&&c.mapped_owned_resolved_chains<=c.samples-c.depth_one&&c.known_rust_plus_other_owned_chains<=c.mapped_owned_resolved_chains&&c.max_frame_depth>=1&&c.max_frame_depth<=Math.min(128,c.frames-c.samples+1)&&c.frames<=c.samples*c.max_frame_depth&&c.max_mapped_owned_resolved_depth<=c.max_frame_depth&&(!c.mapped_owned_resolved_chains||c.max_mapped_owned_resolved_depth>=2)&&((c.max_frame_depth===1)===(c.depth_one===c.samples)));assert(c.sample_span_micros<=Math.min(10,(key==="preflight"?2:r.requested_seconds)+2)*1e6&&c.sample_span_micros<=c.recording_envelope_micros+1&&c.recording_envelope_micros<=c.elapsed_micros);assert(Array.isArray(c.rows)&&c.rows.length===14);
  c.rows.forEach((row,id)=>{closed(row,BIN_KEYS);assert(row.id===id&&Object.values(row).every(uint)&&row.self_samples<=row.inclusive_samples&&row.inclusive_samples<=c.samples&&row.self_period<=row.inclusive_period&&row.inclusive_period<=c.period);});assert(c.rows.reduce((n,row)=>n+row.self_samples,0)===c.samples&&c.rows.reduce((n,row)=>n+row.self_period,0)===c.period&&c.rows[0].self_samples===c.unresolved_self);
 }
 assert(r.available===bit(r.failure_code===0));assert(r.rust_unwind_admitted===bit(r.preflight!==null&&r.preflight.owned_rust_chains>=10));
 if(r.available)assert(!r.failure_code&&!r.failure_stage_code&&!r.loss_observed&&!r.byte_cap_hit&&["synthetic_capable","rust_unwind_admitted","ownership_verified","identity_verified","thread_scope_verified","recorders_stopped","raw_private"].every(k=>r[k]===1)&&(r.preflight_only||r.capture!==null));return r;
}
const PROVENANCE_KEYS=["schema_version","candidate_sha","helper_sha256","elf_sha256","elf_build_id","perf_sha256","appimage_sha256"];
export function validateOwnedPerfProvenance(p){closed(p,PROVENANCE_KEYS);assert(p.schema_version===1);for(const key of PROVENANCE_KEYS.filter(k=>k!=="schema_version"))assert(hex(p[key],key==="candidate_sha"?"40":key==="elf_build_id"?"8,128":"64"));return p;}
const REPORT_KEYS=["schema_version","available","binding_available","invoked","bridge_failure_code","helper_report_available","helper_provenance_bound","profile_phase_code","exact_measurement_window","requested_settle_millis","actual_settle_millis","profile_elapsed_millis","exact_output_caller_available","exact_wait_cpu_available","helper_report"];
function blank(){return {schema_version:1,available:0,binding_available:0,invoked:0,bridge_failure_code:0,helper_report_available:0,helper_provenance_bound:0,profile_phase_code:1,exact_measurement_window:0,requested_settle_millis:SETTLE,actual_settle_millis:0,profile_elapsed_millis:0,exact_output_caller_available:0,exact_wait_cpu_available:0,helper_report:{}};}
export function validateOwnedPerfDiagnostics(r){closed(r,REPORT_KEYS);for(const key of REPORT_KEYS.filter(k=>k!=="helper_report"))assert(uint(r[key]));assert(r.schema_version===1&&r.profile_phase_code===1&&r.exact_measurement_window===0&&r.requested_settle_millis===SETTLE&&r.exact_output_caller_available===0&&r.exact_wait_cpu_available===0&&r.bridge_failure_code<=7);for(const key of ["available","binding_available","invoked","helper_report_available","helper_provenance_bound"])assert(r[key]<=1);if(r.helper_report_available)validateOwnedPerfHelperReport(r.helper_report);else closed(r.helper_report,[]);assert(r.available===bit(r.binding_available&&r.invoked&&!r.bridge_failure_code&&r.helper_report_available&&r.helper_provenance_bound&&r.helper_report.available===1));if(r.invoked)assert(r.actual_settle_millis>=SETTLE);return r;}
export function validateOwnedPerfEnvelope(value,pins){closed(value,["report","provenance"]);validateOwnedPerfHelperReport(value.report);const p=value.provenance;if(p===null){assert(value.report.available===0);return value;}const keys=["schema_version","candidate_sha","helper_sha256","elf_sha256","elf_build_id"];closed(p,Object.hasOwn(p,"perf_sha256")?[...keys,"perf_sha256"]:keys);assert(p.schema_version===1);for(const key of keys.filter(k=>k!=="schema_version"))assert(p[key]===pins[key]);if(Object.hasOwn(p,"perf_sha256"))assert(p.perf_sha256===pins.perf_sha256);if(value.report.available)assert(p.perf_sha256===pins.perf_sha256);return value;}
export async function runOwnedPerfHelper(request,privateDirectory,temporaryRoot,{spawnProcess=spawn}={}){
 const args=["-n","--","/usr/bin/timeout","--signal=TERM","--kill-after=5s","105s","/usr/bin/env",`LIGHT_REPOSITORY_ROOT=${ROOT}`,`LIGHT_ARTIFACTS_DIR=${artifactRoot}`,`LIGHT_TMP_DIR=${temporaryRoot}`,"/usr/bin/python3","-B",HELPER,"--request",request,"--private-directory",privateDirectory];
 return new Promise(resolve=>{let bytes=0,overflow=false,error=false,status=null;const output=[];let child;try{child=spawnProcess("/usr/bin/sudo",args,{cwd:ROOT,stdio:["ignore","pipe","pipe"]});}catch{resolve({status:null,stdout:"",overflow:false,error:true});return;}
  child.stdout.on("data",chunk=>{bytes+=chunk.length;if(bytes>LIMIT)overflow=true;else output.push(chunk);});child.stderr.on("data",()=>{});child.on("error",()=>{error=true;});child.on("close",code=>{status=code;resolve({status,stdout:Buffer.concat(output).toString("utf8"),overflow,error});});
 });
}
export class OwnedPerfBridge {
 constructor({publicDirectory,requestDirectory,temporaryRoot,originalProvenance,environment,runnerPid=process.pid,uid=process.getuid?.()},{io=fs,execute=runOwnedPerfHelper,wait=sleep,now=()=>performance.now(),uuid=()=>crypto.randomUUID()}={}){this.paths={publicDirectory,requestDirectory,temporaryRoot,originalProvenance};this.environment=environment;this.runnerPid=runnerPid;this.uid=uid;this.io=io;this.execute=execute;this.wait=wait;this.now=now;this.uuid=uuid;this.report=blank();this.done=false;this.private={};this.provenance={schema_version:1,candidate_sha:"0".repeat(40),helper_sha256:"0".repeat(64),elf_sha256:"0".repeat(64),elf_build_id:"0".repeat(8),perf_sha256:"0".repeat(64),appimage_sha256:"0".repeat(64)};}
 async initialize(){
  try{for(const value of Object.values(this.paths))safePath(value);const original=await load(this.io,this.paths.originalProvenance,4*1024*1024);await this.io.mkdir(this.paths.publicDirectory,{recursive:true});await this.io.writeFile(path.join(this.paths.publicDirectory,"provenance.json"),original,{mode:0o600,flag:"wx"});const source=JSON.parse(original);assert(hex(source.source_commit,"40")&&source.source_commit===source.workflow_commit);this.provenance.candidate_sha=source.source_commit;
   for(const name of ["tools/desktop-performance-576-owned-perf.py","tools/artifact_paths.py"]){const digest=sha(await load(this.io,path.join(ROOT,name),1024*1024));assert(digest===source.source_sha256[name]);if(name.endsWith("owned-perf.py"))this.provenance.helper_sha256=digest;}
   const config=JSON.parse((await load(this.io,safePath(this.environment.LIGHT_OUTPUT_OWNED_PERF_BINDINGS),16384)).toString("utf8"));closed(config,["schema_version","candidate_sha","appimage_sha256","elf_sha256","elf_build_id","perf_path","perf_sha256"]);assert(config.schema_version===1&&config.candidate_sha===source.source_commit);for(const key of ["appimage_sha256","elf_sha256","perf_sha256"])assert(hex(config[key],"64"));assert(hex(config.elf_build_id,"8,128"));safePath(config.perf_path);assert(path.basename(config.perf_path)==="perf");const images=Object.entries(source.binaries).filter(([name])=>name.endsWith(".AppImage"));assert(images.length===1&&images[0][1].sha256===config.appimage_sha256);
   this.perfPath=config.perf_path;for(const key of ["appimage_sha256","elf_sha256","elf_build_id","perf_sha256"])this.provenance[key]=config[key];assert(uint(this.runnerPid)&&this.runnerPid>0&&uint(this.uid)&&this.uid>0);this.report.binding_available=1;
  }catch{this.report.bridge_failure_code=1;}
  await this.publish();return this;
 }
 async publish(){validateOwnedPerfDiagnostics(this.report);validateOwnedPerfProvenance(this.provenance);await this.io.mkdir(this.paths.publicDirectory,{recursive:true});for(const [name,value] of [["owned-perf-numeric.json",this.report],["owned-perf-provenance.json",this.provenance]]){const file=path.join(this.paths.publicDirectory,name),temporary=file+"."+this.uuid()+".partial";try{await this.io.writeFile(temporary,JSON.stringify(value,null,2)+"\n",{flag:"wx",mode:0o600});await this.io.rename(temporary,file);}finally{await this.io.unlink(temporary).catch(()=>{});}}}
 privateState(){return this.private;}
 async capture(rootPid,processes){
  if(this.done)return;this.done=true;if(!this.report.binding_available)return;
  try{const settleStart=this.now();await this.wait(SETTLE);this.report.actual_settle_millis=Math.ceil(this.now()-settleStart);assert(uint(this.report.actual_settle_millis)&&this.report.actual_settle_millis>=SETTLE);
   const binding=bindOwnedHeadless(processes,rootPid);const identity=async pid=>parseOwnedIdentity((await this.io.readFile(`/proc/${pid}/stat`,"utf8")).toString());const runner=await identity(this.runnerPid),root=await identity(rootPid),target=await identity(binding.pid);assert(runner.pid===this.runnerPid&&root.pid===binding.rootPid&&root.startTicks===binding.rootStartTicks&&root.parentPid===this.runnerPid&&target.pid===binding.pid&&target.startTicks===binding.startTicks);
   const request={schema:1,candidate_sha:this.provenance.candidate_sha,runner_pid:this.runnerPid,runner_start_ticks:runner.startTicks,root_pid:rootPid,root_start_ticks:root.startTicks,target_pid:target.pid,target_start_ticks:target.startTicks,uid:this.uid,elf_sha256:this.provenance.elf_sha256,elf_build_id:this.provenance.elf_build_id,duration_seconds:DURATION,allow_root:1,perf_path:this.perfPath,perf_sha256:this.provenance.perf_sha256};
   const requestFile=path.join(this.paths.requestDirectory,"owned-perf-request.json"),spool=path.join(this.paths.temporaryRoot,"tl775-owned-perf-"+this.uuid());await this.io.writeFile(requestFile,JSON.stringify(request)+"\n",{flag:"wx",mode:0o600});this.private={request,requestFile,spool};this.report.invoked=1;const started=this.now();const result=await this.execute(requestFile,spool,this.paths.temporaryRoot);this.report.profile_elapsed_millis=Math.ceil(this.now()-started);assert(uint(this.report.profile_elapsed_millis));
   if(result.error||result.overflow||![0,1].includes(result.status)){this.report.bridge_failure_code=3;return;}
   let envelope;try{envelope=JSON.parse(result.stdout);validateOwnedPerfEnvelope(envelope,this.provenance);}catch{this.report.bridge_failure_code=4;return;}
   assert(result.status===(envelope.report.available?0:1));assert(envelope.report.preflight_only===0);if(envelope.report.available)assert(envelope.report.requested_seconds===DURATION);
   this.report.helper_report=envelope.report;this.report.helper_report_available=1;this.report.helper_provenance_bound=bit(envelope.provenance!==null&&Object.hasOwn(envelope.provenance,"perf_sha256"));this.report.available=bit(this.report.helper_provenance_bound&&envelope.report.available);
  }catch{this.report.bridge_failure_code ||=2;this.report.available=0;}finally{await this.publish().catch(()=>{});}
 }
}

// Independent synthetic C capability. Never application/Rust admission or cadence proof.
export function validateSyntheticPreflightEnvelope(envelope){
 closed(envelope,["report","provenance"]);const r=envelope.report,p=envelope.provenance;
 const keys=["schema_version","diagnostic_mode_code","available","source_verified","perf_verified","version_available","perf_major","perf_minor","synthetic_capable","raw_private","tools_stopped","failure_code","failure_stage_code","failure_detail_code","loss_observed","byte_cap_hit","elapsed_micros","app_admission","rust_admission","cadence_proof"];
 closed(r,keys);assert(Object.values(r).every(uint));
 assert(r.schema_version===1&&r.diagnostic_mode_code===2&&r.failure_code<=17&&r.failure_stage_code<=14&&r.failure_detail_code<=56&&r.elapsed_micros<=46000000&&r.perf_major<=999&&r.perf_minor<=999);
 for(const key of keys.filter(k=>!["schema_version","diagnostic_mode_code","failure_code","failure_stage_code","failure_detail_code","perf_major","perf_minor","elapsed_micros"].includes(k)))assert(r[key]<=1);
 assert(r.app_admission===0&&r.rust_admission===0&&r.cadence_proof===0);
 assert(!r.failure_detail_code||(r.failure_code===11&&r.failure_stage_code===7));
 assert(r.version_available===bit(r.perf_major>0)&&(r.version_available||r.perf_minor===0)&&r.version_available<=r.perf_verified&&r.perf_verified<=r.source_verified);
 assert(r.loss_observed===bit(r.failure_code===17)&&r.byte_cap_hit===bit(r.failure_code===9)&&r.tools_stopped===bit(r.failure_code!==14));
 assert(r.available===bit(r.failure_code===0&&["source_verified","perf_verified","version_available","synthetic_capable","raw_private","tools_stopped"].every(k=>r[k]===1)));
 assert(!r.available||r.failure_stage_code===0);
 closed(p,["schema_version","candidate_sha","helper_sha256","perf_sha256"]);assert(p.schema_version===1&&hex(p.candidate_sha,40)&&hex(p.helper_sha256,64)&&hex(p.perf_sha256,64));return envelope;
}
