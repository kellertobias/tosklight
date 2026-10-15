import { stripVTControlCharacters } from "node:util";
export const SYNC_REQUEST_DIAGNOSTICS_ENV = "LIGHT_ARCHITECT_SYNC_REQUEST_DIAGNOSTICS";
export const COMMIT_PARTS = ["contract_validation", "store_apply", "feed_publish", "document_apply", "attributes_install", "psn_install", "color_model_install"] as const;
export function syncRequestDiagnosticsEnabled(value: string | undefined) {
  return value === "1" || value === "true";
}
export interface SyncRequestDiagnostic {
  schema: 1;
  ordinal: number;
  success: boolean | null;
  parts: Record<string, {
    calls: number;
  }>;
  [key: string]: unknown;
}

/** Retains numeric opt-in rows independently of the ordinary bounded log tail. */
export class SyncRequestDiagnostics {
  private readonly pending = {
    stdout: "",
    stderr: ""
  };
  private readonly rows: SyncRequestDiagnostic[] = [];
  private errors = 0;
  private dropped = 0;
  private truncated = 0;
  constructor(private readonly enabled: boolean, private readonly capacity = 2048) {}
  feed(stream: "stdout" | "stderr", chunk: string) {
    if (!this.enabled) return;
    const lines = (this.pending[stream] + chunk).split("\n");
    this.pending[stream] = lines.pop() ?? "";
    if (this.pending[stream].length > 65_536) {
      this.pending[stream] = "";
      this.truncated += 1;
    }
    for (const line of lines) this.line(line);
  }
  private line(line: string) {
    const match = /numeric_request=(\{.*\})/.exec(stripVTControlCharacters(line));
    if (!match) return;
    try {
      const value = JSON.parse(match[1]) as SyncRequestDiagnostic;
      if (value.schema !== 1 || !Number.isSafeInteger(value.ordinal) || value.ordinal < 1 || typeof value.parts !== "object" || value.parts === null) throw new Error("invalid diagnostic row");
      if (this.rows.length < this.capacity) this.rows.push(value);else this.dropped += 1;
    } catch {
      this.errors += 1;
    }
  }
  snapshot() {
    return {
      rows: [...this.rows],
      parse_errors: this.errors,
      dropped_rows: this.dropped,
      truncated_lines: this.truncated
    };
  }
}
