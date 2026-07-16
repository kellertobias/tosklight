import { useEffect, useRef, useState } from "react";
import { FileManager, type FileManagerPickerOptions, type FileManagerSelection } from "./FileManagerWindow";

export const OPEN_FILE_MANAGER_PICKER_EVENT = "light:open-file-manager-picker";

export type FileManagerPickerRequest = Omit<FileManagerPickerOptions, "onSelect" | "onCancel">;

interface HostedPickerRequest extends FileManagerPickerRequest {
  onSelect: (selection: FileManagerSelection[]) => void;
  onCancel: () => void;
}

export function openFileManagerPicker(options: FileManagerPickerRequest) {
  return new Promise<FileManagerSelection[] | null>((resolve) => {
    window.dispatchEvent(new CustomEvent<HostedPickerRequest>(OPEN_FILE_MANAGER_PICKER_EVENT, {
      detail: {
        ...options,
        onSelect: (selection) => resolve(selection),
        onCancel: () => resolve(null),
      },
    }));
  });
}

export function FileManagerPickerHost() {
  const [request, setRequest] = useState<HostedPickerRequest | null>(null);
  const requestRef = useRef<HostedPickerRequest | null>(null);
  requestRef.current = request;

  useEffect(() => {
    const open = (event: Event) => {
      const next = (event as CustomEvent<HostedPickerRequest>).detail;
      if (!next || typeof next.onSelect !== "function" || typeof next.onCancel !== "function") return;
      requestRef.current?.onCancel();
      requestRef.current = next;
      setRequest(next);
    };
    window.addEventListener(OPEN_FILE_MANAGER_PICKER_EVENT, open);
    return () => window.removeEventListener(OPEN_FILE_MANAGER_PICKER_EVENT, open);
  }, []);

  if (!request) return null;
  const close = () => {
    requestRef.current = null;
    setRequest(null);
  };
  return <div className="file-picker-backdrop" role="dialog" aria-modal="true" aria-label="Choose files or folders">
    <div className="file-picker-surface">
      <FileManager
        instanceId="hosted-file-picker"
        picker={{
          ...request,
          onSelect: (selection) => {
            request.onSelect(selection);
            close();
          },
          onCancel: () => {
            request.onCancel();
            close();
          },
        }}
      />
    </div>
  </div>;
}
