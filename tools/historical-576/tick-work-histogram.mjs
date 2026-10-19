//! Numeric window bounds for attempted output-tick wall work, not CPU or delivery cadence.
import assert from 'node:assert/strict';

// Identical in f67c84e48baf96a6b8bc667edca7fb859e852ac5 and
// 7a5468514967fca9066c2d00025a3ddb83446d39:
// crates/light/domain/output/src/health.rs OUTPUT_TICK_DURATION_BUCKET_BOUNDS_MICROS.
// scheduler.rs record_tick_duration measures tick().await + result bookkeeping,
// before wait_for_deadline_or_wake. The last band is an overflow sentinel.
export const TICK_WORK_BOUNDS_MICROS = Object.freeze([
  250, 500, 750, 1000, 1250, 1500, 2000, 3000, 4000, 6000,
  8000, 12000, 16000, 24000, 32000, 48000, 64000, 100000,
  1000000, Number.MAX_SAFE_INTEGER,
]);
const KEYS = ['tick_duration_bucket_bounds_micros', 'tick_duration_bucket_counts'];
export const TICK_WORK_FIELDS = Object.freeze([
  'tick_work_histogram_available', 'tick_work_histogram_count',
  ...['p50', 'p95', 'p99'].flatMap(p => [
    `tick_work_${p}_lower_micros`, `tick_work_${p}_upper_micros`,
    `tick_work_${p}_upper_bounded`,
  ]),
]);
const uint = n => Number.isSafeInteger(n) && n >= 0;
const blank = () => Object.fromEntries(TICK_WORK_FIELDS.map(k => [k, 0]));

// Select only the two public histogram fields from an existing diagnostics.output.
// Copy arrays so later caller mutation cannot change an already captured observation.
export function selectTickWorkHistogram(output) {
  return Object.fromEntries(KEYS.map(k => [k, Array.isArray(output?.[k]) ? [...output[k]] : null]));
}
function counts(snapshot) {
  assert(snapshot && typeof snapshot === 'object' && !Array.isArray(snapshot));
  assert.deepEqual(Object.keys(snapshot).sort(), [...KEYS].sort());
  assert.deepEqual(snapshot.tick_duration_bucket_bounds_micros, TICK_WORK_BOUNDS_MICROS);
  const values = snapshot.tick_duration_bucket_counts;
  assert(Array.isArray(values) && values.length === TICK_WORK_BOUNDS_MICROS.length);
  assert(Array.from(values).every(uint));
  assert(uint(values.reduce((a, b) => a + b, 0)));
  return values;
}

// Malformed/reset/empty windows are unavailable, never valid zero-duration work.
// Counts include attempts (including failed/retained output); health result counters
// and duration counts use separate lock acquisitions, so no frame-count equality is imposed.
export function summarizeTickWorkHistogram(snapshots) {
  const result = blank();
  try {
    assert(Array.isArray(snapshots) && snapshots.length === 61);
    const rows = snapshots.map(counts);
    for (let i = 1; i < rows.length; i++)
      assert(rows[i].every((n, j) => n >= rows[i - 1][j]));
    const delta = rows.at(-1).map((n, j) => n - rows[0][j]);
    const total = delta.reduce((a, b) => a + b, 0);
    assert(uint(total) && total > 0);
    result.tick_work_histogram_available = 1;
    result.tick_work_histogram_count = total;
    for (const [p, percent] of [['p50', 50], ['p95', 95], ['p99', 99]]) {
      // Exact nearest-rank integer calculation without unsafe total*percent.
      const rank = Math.floor(total / 100) * percent + Math.ceil((total % 100) * percent / 100);
      let cumulative = 0;
      const index = delta.findIndex(n => { cumulative += n; return cumulative >= rank; });
      const bounded = Number(index < TICK_WORK_BOUNDS_MICROS.length - 1);
      result[`tick_work_${p}_lower_micros`] = index === 0 ? 0 : TICK_WORK_BOUNDS_MICROS[index - 1] + 1;
      result[`tick_work_${p}_upper_micros`] = bounded ? TICK_WORK_BOUNDS_MICROS[index] : 0;
      result[`tick_work_${p}_upper_bounded`] = bounded;
    }
    return result;
  } catch {
    return blank();
  }
}
