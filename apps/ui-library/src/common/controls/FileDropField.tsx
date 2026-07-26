import { useRef, useState, type DragEvent, type ReactNode } from "react";
import { Button, FormField, type LabelPlacement } from "./foundation";

export interface FileDropConstraints {
  extensions?: readonly string[];
  mimeTypes?: readonly string[];
  multiple?: boolean;
  maximumFiles?: number;
}

export type FileDropStatus = "idle" | "loading" | "success" | "error";

export interface FileDropFieldProps {
  label?: ReactNode;
  description?: ReactNode;
  labelPlacement?: LabelPlacement;
  constraints?: FileDropConstraints;
  selectedLabel?: string;
  status?: FileDropStatus;
  statusMessage?: string;
  disabled?: boolean;
  onFiles: (files: readonly File[]) => void;
  onRejected?: (reason: string) => void;
  onOpenPicker: () => void;
}

function normalizedExtension(name: string) {
  const dot = name.lastIndexOf(".");
  return dot < 0 ? "" : name.slice(dot).toLowerCase();
}

export function validateDroppedFiles(
  files: readonly File[],
  constraints: FileDropConstraints = {},
): string | null {
  if (!files.length) return "No supported files were provided.";
  const maximum = constraints.maximumFiles ?? (constraints.multiple ? Number.POSITIVE_INFINITY : 1);
  if (files.length > maximum) return maximum === 1
    ? "Choose one file only."
    : `Choose no more than ${maximum} files.`;
  const extensions = constraints.extensions?.map((extension) =>
    extension.startsWith(".") ? extension.toLowerCase() : `.${extension.toLowerCase()}`);
  const mimeTypes = constraints.mimeTypes?.map((type) => type.toLowerCase());
  const rejected = files.find((file) => {
    const extensionAccepted = Boolean(extensions?.length && extensions.includes(normalizedExtension(file.name)));
    const mimeAccepted = Boolean(mimeTypes?.length && mimeTypes.some((type) =>
      type.endsWith("/*") ? file.type.toLowerCase().startsWith(type.slice(0, -1)) : file.type.toLowerCase() === type));
    return extensions?.length || mimeTypes?.length ? !extensionAccepted && !mimeAccepted : false;
  });
  return rejected ? `${rejected.name} is not an accepted file type.` : null;
}

function constraintLabel(constraints: FileDropConstraints) {
  const types = [
    ...(constraints.extensions ?? []).map((extension) => extension.startsWith(".") ? extension : `.${extension}`),
    ...(constraints.mimeTypes ?? []),
  ];
  const count = constraints.maximumFiles ?? (constraints.multiple ? "multiple" : "one");
  return `${types.length ? types.join(", ") : "supported files"}; ${count} file${count === 1 || count === "one" ? "" : "s"}`;
}

export function FileDropField({
  label,
  description,
  labelPlacement,
  constraints = {},
  selectedLabel,
  status = "idle",
  statusMessage,
  disabled = false,
  onFiles,
  onRejected,
  onOpenPicker,
}: FileDropFieldProps) {
  const [dragState, setDragState] = useState<"idle" | "accepted" | "rejected">("idle");
  const [rejection, setRejection] = useState<string | null>(null);
  const dragDepth = useRef(0);
  const inspect = (event: DragEvent<HTMLElement>) => {
    const files = [...event.dataTransfer.files];
    return validateDroppedFiles(files, constraints);
  };
  const enter = (event: DragEvent<HTMLElement>) => {
    event.preventDefault();
    dragDepth.current += 1;
    const reason = inspect(event);
    setRejection(reason);
    setDragState(reason ? "rejected" : "accepted");
  };
  const over = (event: DragEvent<HTMLElement>) => {
    event.preventDefault();
    const reason = inspect(event);
    event.dataTransfer.dropEffect = reason ? "none" : "copy";
    setRejection(reason);
    setDragState(reason ? "rejected" : "accepted");
  };
  const leave = (event: DragEvent<HTMLElement>) => {
    event.preventDefault();
    dragDepth.current = Math.max(0, dragDepth.current - 1);
    if (!dragDepth.current) setDragState("idle");
  };
  const drop = (event: DragEvent<HTMLElement>) => {
    event.preventDefault();
    dragDepth.current = 0;
    const files = [...event.dataTransfer.files];
    const reason = validateDroppedFiles(files, constraints);
    setDragState("idle");
    setRejection(reason);
    if (reason) onRejected?.(reason);
    else onFiles(files);
  };
  const message = dragState === "accepted"
    ? "Release to choose these files."
    : dragState === "rejected"
      ? rejection
      : statusMessage ?? selectedLabel ?? "Drop files here or open ToskLight File Manager";
  const fieldName = typeof label === "string" ? `${label}. ` : "";
  return <FormField label={label} description={description} labelPlacement={labelPlacement}>
    <Button
      className={`ui-file-drop-field drag-${dragState} status-${status}`}
      disabled={disabled}
      aria-label={`Choose file. ${fieldName}${constraintLabel(constraints)}. ${message ?? ""}`}
      aria-describedby={undefined}
      onClick={onOpenPicker}
      onDragEnter={enter}
      onDragOver={over}
      onDragLeave={leave}
      onDrop={drop}
    >
      <span className="ui-file-drop-icon" aria-hidden="true">⇩</span>
      <span className="ui-file-drop-copy">
        <b>{message}</b>
        <small>{constraintLabel(constraints)}</small>
      </span>
      <span className="ui-file-drop-browse">Browse File Manager</span>
    </Button>
    {status === "error" && statusMessage && <small className="ui-file-drop-error" role="alert">{statusMessage}</small>}
  </FormField>;
}
