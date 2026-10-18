// Old-frontend compile compatibility only: one erased type annotation, no timer behavior changes.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {lstat,open,constants} from 'node:fs/promises';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
export const SOURCE_SHA='18db82fddd820e898995ee1ff962d27c9d43810ca7dda376cdaf882548df82f9';
export const OUTPUT_SHA='8dfeb552e904e1f1af78a2c4e6ec29a954f52fc3d6d768bb3d9fa33ed47a973f';
const OLD='\ttimer: number;',NEW='\ttimer: ReturnType<typeof globalThis.setTimeout>;';
const digest=bytes=>createHash('sha256').update(bytes).digest('hex');
export function transform(source){
  assert.equal(digest(source),SOURCE_SHA);const text=source.toString('utf8');assert.equal(text.split(OLD).length,2);
  const result=Buffer.from(text.replace(OLD,NEW));assert.equal(digest(result),OUTPUT_SHA);return result;
}
export async function overlay(source,apply=false){
  assert(path.isAbsolute(source)&&source.endsWith('/apps/light-desktop/src/api/client/runtime.ts'));assert.equal(typeof apply,'boolean');
  const info=await lstat(source);assert(info.isFile()&&!info.isSymbolicLink()&&info.size>0&&info.size<=65536);
  const file=await open(source,(apply?constants.O_RDWR:constants.O_RDONLY)|constants.O_NOFOLLOW);
  try{
    const actual=await file.stat();assert(actual.isFile()&&actual.dev===info.dev&&actual.ino===info.ino&&actual.size===info.size);
    const before=await file.readFile();assert(before.length===actual.size);const after=transform(before);
    if(apply){let offset=0;while(offset<after.length){const written=await file.write(after,offset,after.length-offset,offset);assert(written.bytesWritten>0);offset+=written.bytesWritten;}await file.truncate(after.length);await file.sync();}
    return {schema_version:1,base_sha256:SOURCE_SHA,patched_sha256:OUTPUT_SHA,applied:Number(apply),type_only:1,runtime_statements_changed:0};
  }finally{await file.close();}
}
if(process.argv[1]&&path.resolve(process.argv[1])===fileURLToPath(import.meta.url)){
  const args=process.argv.slice(2);assert(args.length===2||args.length===3);assert.equal(args[0],'--source');assert(args.length===2||args[2]==='--apply');
  console.log(JSON.stringify(await overlay(args[1],args[2]==='--apply')));
}
