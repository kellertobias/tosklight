// Private benchmark-surface-only shim; no engine, Dynamics, or backend changes.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
export const OLD_REVISION = 'f67c84e48baf96a6b8bc667edca7fb859e852ac5';
export const OLD_BENCHMARK_SHA256 = 'd348e549f88741561a4f66a7ea4a0b0798a717afeb2d077f87034e6b61a3360b';
export const OLD_CONTROL_SHA256 = 'eb7211d23b11717cdd8a807b998b0868c2545938c4e485b54693928f7f15eed6';
export const CONTROL_PROFILE = 'historical-control-576';
export function oldBenchmarkControlShim(source) {
  assert.equal(createHash('sha256').update(source).digest('hex'), OLD_BENCHMARK_SHA256);
  const replacements = [
    ['const [stageEnabled, setStageEnabled] = useState(false);', 'const [stageEnabled, setStageEnabled] = useState(false);\n\tconst controlOnly = config.profile === "historical-control-576";'],
    ['const beginStage = window.setTimeout(() => {', 'const beginStage = window.setTimeout(() => {\n\t\t\tif (controlOnly) return;'],
    ['<FixtureSheetWindow compact active={stageEnabled} />', '<FixtureSheetWindow compact active={controlOnly ? prepared : stageEnabled} />'],
  ];
  for (const [before, after] of replacements) {
    assert.equal(source.split(before).length, 2, 'old benchmark seam changed');
    source = source.replace(before, after);
  }
  return source;
}

// Builder-only explicit apply: exactly the hash-pinned old benchmark surface file.
if (process.argv[1] && new URL(import.meta.url).pathname === process.argv[1]) {
  const {readFile, writeFile, lstat} = await import('node:fs/promises');
  const path = await import('node:path');
  const args = process.argv.slice(2);
  assert(args.length === 2 || args.length === 3);
  assert.equal(args[0], '--source');
  assert(path.isAbsolute(args[1]) && args[1].endsWith('/apps/light-desktop/src/NativePackagedStageBenchmarkApp.tsx'));
  assert(args.length === 2 || args[2] === '--apply');
  const stat = await lstat(args[1]); assert(stat.isFile() && !stat.isSymbolicLink() && stat.size < 65536);
  const original = await readFile(args[1], 'utf8');
  const transformed = oldBenchmarkControlShim(original);
  assert.equal(createHash('sha256').update(transformed).digest('hex'), OLD_CONTROL_SHA256);
  if (args[2] === '--apply') await writeFile(args[1], transformed);
  console.log(JSON.stringify({schema_version:1,base_sha256:OLD_BENCHMARK_SHA256,patched_sha256:OLD_CONTROL_SHA256,applied:Number(args[2] === '--apply')}));
}
