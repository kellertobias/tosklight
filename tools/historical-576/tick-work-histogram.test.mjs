import assert from 'node:assert/strict';
import test from 'node:test';
import {TICK_WORK_BOUNDS_MICROS as B, TICK_WORK_FIELDS, selectTickWorkHistogram, summarizeTickWorkHistogram as summarize} from './tick-work-histogram.mjs';
const row = counts => ({tick_duration_bucket_bounds_micros: [...B], tick_duration_bucket_counts: counts});
function window(delta, initial = Array(20).fill(0)) {
  return Array.from({length: 61}, (_, i) => row(initial.map((n, j) => n + (i === 60 ? delta[j] : 0))));
}
const unavailable = rows => { const r = summarize(rows); assert.deepEqual(Object.keys(r), TICK_WORK_FIELDS); assert(Object.values(r).every(n => n === 0)); };
test('producer contract is exactly twenty fixed inclusive bands', () => {
  assert.deepEqual(B, [250,500,750,1000,1250,1500,2000,3000,4000,6000,8000,12000,16000,24000,32000,48000,64000,100000,1000000,9007199254740991]);
});
test('nearest ranks use exact inclusive bucket boundaries and never midpoints', () => {
  const d=Array(20).fill(0); d[0]=50; d[11]=45; d[13]=4; d[18]=1;
  const r=summarize(window(d,Array(20).fill(100)));
  assert.equal(r.tick_work_histogram_count,100); assert.equal(r.tick_work_histogram_available,1);
  for(const [p,lo,hi] of [['p50',0,250],['p95',8001,12000],['p99',16001,24000]]) {
    assert.equal(r[`tick_work_${p}_lower_micros`],lo); assert.equal(r[`tick_work_${p}_upper_micros`],hi); assert.equal(r[`tick_work_${p}_upper_bounded`],1);
  }
});
test('one observation and exact rank crossing select containing integer-microsecond band', () => {
  for(let j=0;j<19;j++){const d=Array(20).fill(0);d[j]=1;const r=summarize(window(d));for(const p of ['p50','p95','p99']){assert.equal(r[`tick_work_${p}_lower_micros`],j?B[j-1]+1:0);assert.equal(r[`tick_work_${p}_upper_micros`],B[j]);}}
  const d=Array(20).fill(0);d[0]=49;d[1]=51;assert.equal(summarize(window(d)).tick_work_p50_lower_micros,251);
});
test('overflow is explicitly unbounded, with numeric unknown upper zero', () => {
  const d=Array(20).fill(0);d[19]=1;const r=summarize(window(d));assert.equal(r.tick_work_histogram_available,1);
  for(const p of ['p50','p95','p99']){assert.equal(r[`tick_work_${p}_lower_micros`],1000001);assert.equal(r[`tick_work_${p}_upper_micros`],0);assert.equal(r[`tick_work_${p}_upper_bounded`],0);}
  assert(Object.values(r).every(Number.isSafeInteger));
});
test('zero window, wrong observation count and malformed rows are unavailable', () => {
  unavailable(window(Array(20).fill(0)));unavailable([]);unavailable(Array(62).fill(row(Array(20).fill(0))));
  const d=Array(20).fill(0);d[0]=1;
  for(const change of [x=>delete x.tick_duration_bucket_counts,x=>x.path='/private',x=>x.tick_duration_bucket_counts.pop(),x=>x.tick_duration_bucket_bounds_micros[0]++,x=>x.tick_duration_bucket_counts[0]=-1,x=>x.tick_duration_bucket_counts[0]=.5,x=>x.tick_duration_bucket_counts[0]=NaN,x=>x.tick_duration_bucket_counts[0]=Number.MAX_SAFE_INTEGER+1]){const w=window(d);change(w[10]);unavailable(w);}
});
test('any intermediate bucket decrease/reset fails even when endpoints look increasing', () => {
  const d=Array(20).fill(0);d[0]=5;const w=window(d,Array(20).fill(10));w[30].tick_duration_bucket_counts[4]=9;unavailable(w);
});
test('safe count totals are required and large safe ranks remain exact', () => {
  const d=Array(20).fill(0);d[0]=Number.MAX_SAFE_INTEGER;assert.equal(summarize(window(d)).tick_work_histogram_count,Number.MAX_SAFE_INTEGER);
  d[1]=1;unavailable(window(d));
});
test('selector copies only histogram fields and cannot leak paths or other health values', () => {
  const output={...row(Array(20).fill(0)),path:'/private',last_tick_micros:999};const selected=selectTickWorkHistogram(output);
  assert.deepEqual(Object.keys(selected).sort(),['tick_duration_bucket_bounds_micros','tick_duration_bucket_counts']);output.tick_duration_bucket_counts[0]=3;assert.equal(selected.tick_duration_bucket_counts[0],0);
  unavailable(Array(61).fill(selectTickWorkHistogram(null)));
});
