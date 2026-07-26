import { useRef, useState } from "react";
import {
  FileDropField,
  type FileDropConstraints,
  type FileDropStatus,
} from "@tosklight/ui/controls";
import { RootConfinedFilePickerButton } from "./RootConfinedFilePickerButton";

export interface RootConfinedFileDropFieldProps {
  label: string;
  constraints?: FileDropConstraints;
  disabled?: boolean;
  onFiles: (files: readonly File[]) => void | Promise<void>;
}

export function RootConfinedFileDropField({
  label,
  constraints,
  disabled = false,
  onFiles,
}: RootConfinedFileDropFieldProps) {
  const trigger = useRef<(() => void) | null>(null);
  const [status, setStatus] = useState<FileDropStatus>("idle");
  const [message, setMessage] = useState("");
  const accept = async (files: readonly File[]) => {
    setStatus("loading");
    setMessage("Loading selected file…");
    try {
      await onFiles(files);
      setStatus("success");
      setMessage(files.length === 1 ? files[0].name : `${files.length} files selected`);
    } catch (reason) {
      setStatus("error");
      setMessage(`Could not use the selected file: ${String(reason)}`);
    }
  };
  return <>
    <FileDropField
      label={label}
      constraints={constraints}
      disabled={disabled}
      status={status}
      statusMessage={message}
      onFiles={accept}
      onRejected={(reason) => {
        setStatus("error");
        setMessage(reason);
      }}
      onOpenPicker={() => trigger.current?.()}
    />
    <RootConfinedFilePickerButton
      label={label}
      allowedExtensions={constraints?.extensions ? [...constraints.extensions] : undefined}
      multiple={constraints?.multiple}
      disabled={disabled}
      hideButton
      triggerRef={trigger}
      onFiles={accept}
    />
  </>;
}
