import { formatErrorDetails } from "@tosklight/ui";
import type { RefObject } from "react";
import type { TimecodeDefinition } from "../../api/types/timecode";
import { parseMarkerCsv } from "./editorModel";
export async function importTimecodeMarkerCsv(file: File, { fps, duration, mode, latestDraft, setDraft, setError }: {
 fps: number; duration: number; mode: string; latestDraft: RefObject<TimecodeDefinition>;
 setDraft(definition: TimecodeDefinition): void; setError(error: string | null): void;
}) {
 try {
  const imported = parseMarkerCsv(await file.text(), fps, Math.max(1, duration));
  setDraft({ ...latestDraft.current, markers: mode === "append" ? [...latestDraft.current.markers, ...imported] : imported });
  setError(null);
 } catch (reason) { setError(formatErrorDetails(reason)); }
}
