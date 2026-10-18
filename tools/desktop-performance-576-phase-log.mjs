// Freeze one bounded private byte prefix. Existing collectors retain their own caps/parsers.
import assert from 'node:assert/strict';
import * as fs from 'node:fs/promises';
import {constants} from 'node:fs';
import path from 'node:path';
export const PHASE_LOG_BYTE_CAP=32*1024*1024;
export async function freezePhaseLog(source,destination,{io=fs}={}){
 assert(typeof source==='string'&&path.isAbsolute(source));
 assert(typeof destination==='string'&&path.isAbsolute(destination)&&source!==destination);
 let reader,writer,created=false;
 try{
  reader=await io.open(source,constants.O_RDONLY|constants.O_NOFOLLOW);
  const info=await reader.stat();assert(info.isFile()&&Number.isSafeInteger(info.size)&&info.size>=0);
  // CAP+1 is the existing readers' overcap sentinel, never an increased admission limit.
  const size=Math.min(info.size,PHASE_LOG_BYTE_CAP+1),bytes=Buffer.alloc(size);let used=0;
  while(used<size){const part=await reader.read(bytes,used,size-used,used);assert(Number.isSafeInteger(part.bytesRead)&&part.bytesRead>0&&part.bytesRead<=size-used);used+=part.bytesRead;}
  writer=await io.open(destination,constants.O_WRONLY|constants.O_CREAT|constants.O_EXCL|constants.O_NOFOLLOW,0o600);created=true;
  await writer.writeFile(bytes);await writer.close();writer=null;await reader.close();reader=null;
  return destination;
 }catch(error){if(created)await io.unlink(destination).catch(()=>{});throw error;}
 finally{await writer?.close().catch(()=>{});await reader?.close().catch(()=>{});}
}
