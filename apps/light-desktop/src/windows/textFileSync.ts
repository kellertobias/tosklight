import type { FileOperationResult, TextDocument } from "../api/types";

export const TEXT_FILE_SAVED_EVENT = "light:text-file-saved";
export const TEXT_FILE_OPERATION_EVENT = "light:file-operation";

export type TextFileSavedEventDetail =
  | TextDocument
  | {
      document: TextDocument;
      sourcePaneId?: string;
    };

export function textDocumentFromSavedEvent(event: Event): { document: TextDocument; sourcePaneId?: string } | null {
  const detail = (event as CustomEvent<TextFileSavedEventDetail>).detail;
  if (!detail || typeof detail !== "object") return null;
  if ("document" in detail) return detail;
  if ("root_id" in detail && "path" in detail && "revision" in detail) return { document: detail };
  return null;
}

export function publishTextFileSaved(document: TextDocument, sourcePaneId?: string) {
  window.dispatchEvent(new CustomEvent<TextFileSavedEventDetail>(TEXT_FILE_SAVED_EVENT, {
    detail: { document, sourcePaneId },
  }));
}

export type TextFileOperationKind = "create_file" | "create_folder" | "rename" | "copy" | "move" | "trash" | "delete";

export interface TextFileOperationEventDetail {
  operation: TextFileOperationKind;
  items: FileOperationResult["items"];
  sourceInstanceId?: string;
}

export type TextFileLocationChange =
  | { kind: "moved"; rootId: string; path: string }
  | { kind: "deleted" };

export function publishTextFileOperation(
  operation: TextFileOperationKind,
  result: FileOperationResult,
  sourceInstanceId?: string,
) {
  window.dispatchEvent(new CustomEvent<TextFileOperationEventDetail>(TEXT_FILE_OPERATION_EVENT, {
    detail: { operation, items: result.items, sourceInstanceId },
  }));
}

export function textFileOperationFromEvent(event: Event): TextFileOperationEventDetail | null {
  const detail = (event as CustomEvent<unknown>).detail;
  if (!detail || typeof detail !== "object") return null;
  const candidate = detail as Partial<TextFileOperationEventDetail>;
  if (typeof candidate.operation !== "string" || !Array.isArray(candidate.items)) return null;
  return candidate as TextFileOperationEventDetail;
}

/**
 * Map a completed operation on a file or one of its parent folders to the
 * document's new location. Copies leave the original association unchanged.
 */
export function textFileLocationChange(
  rootId: string,
  path: string,
  detail: TextFileOperationEventDetail,
): TextFileLocationChange | null {
  if (detail.operation === "copy" || detail.operation === "create_file" || detail.operation === "create_folder") return null;
  for (const item of detail.items) {
    if (item.status !== "completed" || item.source_root_id !== rootId) continue;
    const descendant = path === item.source
      ? ""
      : path.startsWith(`${item.source}/`)
        ? path.slice(item.source.length + 1)
        : null;
    if (descendant == null) continue;
    if (detail.operation === "delete" || detail.operation === "trash") return { kind: "deleted" };
    if ((detail.operation === "rename" || detail.operation === "move") && item.destination_root_id && item.destination != null) {
      return {
        kind: "moved",
        rootId: item.destination_root_id,
        path: descendant ? `${item.destination}/${descendant}` : item.destination,
      };
    }
  }
  return null;
}
