import test from 'node:test';
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {readFile,mkdir,mkdtemp,writeFile,stat,symlink,rm} from 'node:fs/promises';
import {pathToFileURL} from 'node:url';
import path from 'node:path';
import {transform,overlay,SOURCE_SHA,OUTPUT_SHA} from './old-runtime-timer-type.mjs';
import {createHash} from 'node:crypto';
const ROOT=path.resolve(process.cwd());
const {artifactPaths}=await import(pathToFileURL(path.join(ROOT,'tools/artifact-paths.mjs')).href);
const before=execFileSync('git',['show','f67c84e48baf96a6b8bc667edca7fb859e852ac5:apps/light-desktop/src/api/client/runtime.ts'],{cwd:ROOT,maxBuffer:65536});
const digest=x=>createHash('sha256').update(x).digest('hex');
async function fixture(run){await mkdir(artifactPaths.tmp,{recursive:true});const root=await mkdtemp(path.join(artifactPaths.tmp,'historical-timer-pure-'));try{const p=path.join(root,'apps/light-desktop/src/api/client/runtime.ts');await mkdir(path.dirname(p),{recursive:true});await writeFile(p,before,{mode:0o640});await run(p,root);}finally{await rm(root,{recursive:true,force:true});}}
test('exact frozen input/output and only one type annotation changes',()=>{assert.equal(digest(before),SOURCE_SHA);const after=transform(before);assert.equal(digest(after),OUTPUT_SHA);assert.equal(after.toString().replace('\ttimer: ReturnType<typeof globalThis.setTimeout>;','\ttimer: number;'),before.toString());for(const token of ['globalThis.setTimeout(', 'globalThis.clearTimeout('])assert.equal(after.toString().split(token).length,before.toString().split(token).length);});
test('unknown source, extra occurrence and already patched bytes refuse',()=>{for(const x of [Buffer.alloc(0),Buffer.concat([before,Buffer.from('\n')]),Buffer.concat([before,Buffer.from('\ttimer: number;')]),transform(before)])assert.throws(()=>transform(x));});
test('default attestation does not write; explicit apply preserves mode',async()=>fixture(async p=>{const originalMode=(await stat(p)).mode&0o777;const r=await overlay(p);assert.equal(r.applied,0);assert.deepEqual(await readFile(p),before);assert.equal((await overlay(p,true)).applied,1);assert.equal((await stat(p)).mode&0o777,originalMode);assert.deepEqual(await readFile(p),transform(before));await assert.rejects(overlay(p,true));}));
test('symlink and wrong absolute target refuse without changing original',async()=>fixture(async(p,root)=>{const q=path.join(root,'link/apps/light-desktop/src/api/client/runtime.ts');await mkdir(path.dirname(q),{recursive:true});await symlink(p,q);await assert.rejects(overlay(q,true));await assert.rejects(overlay('relative/apps/light-desktop/src/api/client/runtime.ts',true));await assert.rejects(overlay(path.join(root,'other.ts'),true));assert.deepEqual(await readFile(p),before);}));
test('source mismatch never writes',async()=>fixture(async p=>{await writeFile(p,'WRONG');await assert.rejects(overlay(p,true));assert.equal((await readFile(p,'utf8')),'WRONG');}));
test('CLI defaults to attestation and accepts only explicit apply',async()=>fixture(async p=>{const helper=new URL('./old-runtime-timer-type.mjs',import.meta.url);const r=JSON.parse(execFileSync('node',[helper.pathname,'--source',p],{encoding:'utf8'}));assert.equal(r.applied,0);assert.deepEqual(await readFile(p),before);const a=JSON.parse(execFileSync('node',[helper.pathname,'--source',p,'--apply'],{encoding:'utf8'}));assert.equal(a.applied,1);assert.deepEqual(await readFile(p),transform(before));}));
