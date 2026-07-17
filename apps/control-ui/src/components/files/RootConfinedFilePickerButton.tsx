import { useCallback, useEffect, useState, type MutableRefObject } from "react";
import { useServer } from "../../api/ServerContext";
import { Button, type ButtonProps } from "../common/controls";
import { openFileManagerPicker } from "../../windows/FileManagerPickerHost";

export interface RootConfinedFilePickerButtonProps {
  label: string;
  allowedExtensions?: string[];
  multiple?: boolean;
  disabled?: boolean;
  buttonClassName?: string;
  variant?: ButtonProps["variant"];
  hideButton?: boolean;
  triggerRef?: MutableRefObject<(() => void) | null>;
  onFiles: (files: File[]) => void | Promise<void>;
}

export function RootConfinedFilePickerButton({
  label,
  allowedExtensions,
  multiple = false,
  disabled = false,
  buttonClassName,
  variant,
  hideButton = false,
  triggerRef,
  onFiles,
}: RootConfinedFilePickerButtonProps) {
  const server = useServer();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  const choose = useCallback(async () => {
    setError("");
    const result = await openFileManagerPicker({ purpose: label, target: "files", multiple, allowedExtensions });
    if (!result) return;
    setBusy(true);
    try {
      const files = Array.isArray(result)
        ? await Promise.all(result.map(async ({ rootId, entry }) => {
          const content = await server.fileContent(rootId, entry.path);
          return new File([content], entry.name, { type: content.type, lastModified: entry.modified_millis ?? Date.now() });
        }))
        : result.files;
      await onFiles(files);
    } catch (reason) {
      setError(`Could not use the selected file: ${String(reason)}`);
    } finally {
      setBusy(false);
    }
  }, [allowedExtensions, label, multiple, onFiles, server]);

  useEffect(() => {
    if (!triggerRef) return;
    triggerRef.current = () => void choose();
    return () => { triggerRef.current = null; };
  }, [choose, triggerRef]);

  return <span className="root-confined-file-picker">
    {!hideButton && <Button aria-label={label} variant={variant} className={buttonClassName} disabled={disabled || busy} onClick={() => void choose()}>{busy ? "Loading selected file…" : label}</Button>}
    {error && <small role="alert">{error}</small>}
  </span>;
}
