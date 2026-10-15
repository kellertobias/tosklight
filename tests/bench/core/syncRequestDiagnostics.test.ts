import { describe, expect, it } from "vitest";
import { SyncRequestDiagnostics, syncRequestDiagnosticsEnabled } from "./syncRequestDiagnostics";
const row = (ordinal: number) => `INFO numeric_request=${JSON.stringify({
  schema: 1,
  ordinal,
  success: true,
  parts: {
    store_apply: {
      calls: 1
    }
  }
})} isolated correlated sync request\n`;
describe("bounded correlated Architect diagnostics", () => {
  it("is disabled unless explicitly opted in", () => {
    for (const value of [undefined, "", "0", "TRUE", "true "]) expect(syncRequestDiagnosticsEnabled(value)).toBe(false);
    expect(syncRequestDiagnosticsEnabled("1")).toBe(true);
    expect(syncRequestDiagnosticsEnabled("true")).toBe(true);
    const collector = new SyncRequestDiagnostics(false);
    collector.feed("stderr", row(1));
    expect(collector.snapshot().rows).toEqual([]);
  });
  it("retains all1000 rows despite fragmentation and independent stream tails", () => {
    const collector = new SyncRequestDiagnostics(true);
    for (let i = 1; i <= 1000; i++) {
      const text = row(i);
      collector.feed("stderr", text.slice(0, 23));
      collector.feed("stdout", "unrelated stdout\n");
      collector.feed("stderr", text.slice(23));
    }
    expect(collector.snapshot().rows.map(r => r.ordinal)).toEqual(Array.from({
      length: 1000
    }, (_, i) => i + 1));
    expect(collector.snapshot().parse_errors).toBe(0);
  });
  it("rejects malformed rows and bounds retained evidence", () => {
    const collector = new SyncRequestDiagnostics(true, 2);
    collector.feed("stderr", row(1) + "numeric_request={broken}\n" + row(2) + row(3));
    expect(collector.snapshot().rows).toHaveLength(2);
    expect(collector.snapshot().parse_errors).toBe(1);
    expect(collector.snapshot().dropped_rows).toBe(1);
  });
  it("bounds an unterminated line and recovers on the following complete row", () => {
    const collector = new SyncRequestDiagnostics(true);
    collector.feed("stderr", "x".repeat(65537));
    collector.feed("stderr", "\n" + row(4));
    expect(collector.snapshot().truncated_lines).toBe(1);
    expect(collector.snapshot().rows[0].ordinal).toBe(4);
  });
});
