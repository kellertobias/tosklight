import assert from 'node:assert/strict';
import test from 'node:test';
import { mkdirSync, mkdtempSync, writeFileSync, existsSync, rmSync } from 'node:fs';
import { resolve } from 'node:path';
import sharp from 'sharp';
import { artifactPaths } from './artifact-paths.mjs';
import { marketingWebImages, responsiveScreenshot } from './marketing-web-images.mjs';

test('4K sources become bounded mobile JPEGs and reused PNGs disappear', async () => {
  mkdirSync(artifactPaths.tmp, { recursive: true });
  const root = mkdtempSync(resolve(artifactPaths.tmp, 'web-images-'));
  try {
    const source = resolve(root, 'source'); const destination = resolve(root, 'site');
    mkdirSync(source); mkdirSync(destination);
    await sharp({create:{width:3840,height:2160,channels:3,background:'#183050'}}).png().toFile(resolve(source,'venue.png'));
    writeFileSync(resolve(destination,'previous-4k.png'), 'stale capture');
    const images = await marketingWebImages([{file:'venue.png'}], source, destination);
    const image = images.get('venue.png');
    assert.deepEqual(image.variants.map(v=>[v.width,v.height]),[[640,360],[960,540],[1600,900]]);
    assert.equal(existsSync(resolve(destination,'previous-4k.png')),false);
    for (const variant of image.variants) {
      const metadata = await sharp(resolve(destination,variant.file)).metadata();
      assert.equal(metadata.format,'jpeg'); assert.ok(metadata.width<=1600);
      assert.ok(variant.bytes<100_000);
    }
    const html = responsiveScreenshot(image);
    assert.match(html,/srcset=.*640w.*960w.*1600w/u);
    assert.doesNotMatch(html,/\.png/u);
    assert.equal((await sharp(resolve(source,'venue.png')).metadata()).width,3840);
  } finally { rmSync(root,{recursive:true,force:true}); }
});
