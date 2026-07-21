import { useEffect, useRef, useState } from "react";
import { useFiles } from "../features/files/FilesContext";
import { Button, Input, ModalTitleBar } from "../components/common";
import { PaneChromeProvider } from "../components/shell/PaneChromeContext";
import { extension, FileManager } from "./FileManagerWindow";
import {
	attachControllableHostedPicker,
	controllableHostedPickerOutcome,
} from "./fileManagerWindow/controllableHostedPicker";
import {
	createHostedPickerOperation,
	type FileManagerPickerRequest,
	type HostedPickerRequest,
} from "./fileManagerWindow/hostedPickerContract";

export const OPEN_FILE_MANAGER_PICKER_EVENT = "light:open-file-manager-picker";

export type {
	FileManagerPickerRequest,
	HostedFileManagerPickerResult,
	SystemFilePickerSelection,
} from "./fileManagerWindow/hostedPickerContract";

export function openFileManagerPicker(options: FileManagerPickerRequest) {
	const operation = createHostedPickerOperation(options);
	window.dispatchEvent(
		new CustomEvent<HostedPickerRequest>(OPEN_FILE_MANAGER_PICKER_EVENT, {
			detail: operation.request,
		}),
	);
	return operation.result;
}

export function FileManagerPickerHost() {
  const files = useFiles();
  const [request, setRequest] = useState<HostedPickerRequest | null>(null);
  const [systemError, setSystemError] = useState("");
  const [chromeInfo, setChromeInfo] = useState<HTMLSpanElement | null>(null);
  const [chromeToolbar, setChromeToolbar] = useState<HTMLSpanElement | null>(null);
  const requestRef = useRef<HostedPickerRequest | null>(null);
  const systemInput = useRef<HTMLInputElement | null>(null);

  useEffect(() => {
		const accept = (next: HostedPickerRequest) => {
			requestRef.current?.onCancel();
			requestRef.current = next;
			setSystemError("");
			setRequest(next);
		};
		const cancel = (target: HostedPickerRequest) => {
			if (requestRef.current !== target) return;
			requestRef.current = null;
			setRequest((current) => (current === target ? null : current));
			target.onCancel();
		};
		const open = (event: Event) => {
			const next = normalizeHostedPickerRequest(
				(event as CustomEvent<unknown>).detail,
			);
			if (next) accept(next);
		};
		const detachControl = attachControllableHostedPicker((options) => {
			const operation = createHostedPickerOperation(options);
			accept(operation.request);
			return {
				outcome: operation.result.then(controllableHostedPickerOutcome),
				cancel: () => cancel(operation.request),
			};
		});
    window.addEventListener(OPEN_FILE_MANAGER_PICKER_EVENT, open);
		return () => {
			window.removeEventListener(OPEN_FILE_MANAGER_PICKER_EVENT, open);
			detachControl();
			const active = requestRef.current;
			requestRef.current = null;
			active?.onCancel();
		};
  }, []);

  if (!request) return null;
	const complete = (callback: () => void) => {
		if (requestRef.current !== request) return;
		requestRef.current = null;
		setRequest(null);
		callback();
	};
  const target = request.target ?? "files";
  const purpose = request.purpose ?? (target === "folders"
    ? request.multiple ? "Select folders" : "Select a folder"
    : target === "either"
      ? request.multiple ? "Select files or folders" : "Select a file or folder"
      : request.multiple ? "Select files" : "Select a file");
  const allowedExtensions = (request.allowedExtensions ?? []).map((value) => value.replace(/^\./, "").toLowerCase()).filter(Boolean);
  const accept = allowedExtensions.map((value) => `.${value}`).join(",");
  const setSystemInput = (input: HTMLInputElement | null) => {
    systemInput.current = input;
    if (input) {
      if (target === "folders") input.setAttribute("webkitdirectory", "");
      else input.removeAttribute("webkitdirectory");
    }
  };
  const systemSelected = (files: File[]) => {
    if (!files.length) return;
    const selected = request.multiple || target === "folders" ? files : files.slice(0, 1);
    if (target !== "folders" && allowedExtensions.length && selected.some((file) => !allowedExtensions.includes(extension(file.name)))) {
      setSystemError(`Choose only ${allowedExtensions.map((value) => `.${value}`).join(", ")} files.`);
      return;
    }
		complete(() =>
			request.onSystemSelect({
				source: "system",
				target,
				files: selected,
				...(target === "folders"
					? {
							directoryName:
								selected[0]?.webkitRelativePath.split("/")[0] || undefined,
						}
					: {}),
			}),
		);
  };
  return <div className="file-picker-backdrop" role="dialog" aria-modal="true" aria-label="Choose files or folders">
    <div className="file-picker-surface">
      <ModalTitleBar
        title="File Manager"
        details={<><b>{purpose}</b><small><span className="pane-chrome-info-target" ref={setChromeInfo} /></small></>}
        actions={<span className="pane-chrome-toolbar-target" ref={setChromeToolbar} />}
        closeLabel="Close File Manager"
		onClose={() => complete(request.onCancel)}
      />
      <PaneChromeProvider value={{ info: chromeInfo, toolbar: chromeToolbar }}>
        <FileManager
          instanceId="hosted-file-picker"
          picker={{
            ...request,
			onSelect: (selection) => complete(() => request.onSelect(selection)),
			onCancel: () => complete(request.onCancel),
          }}
        />
      </PaneChromeProvider>
      {files.systemPickerFallback && <footer className="file-picker-system-fallback">
        <Button onClick={() => systemInput.current?.click()}>Open system file picker</Button>
        <small>This secondary picker keeps the calling form's target, selection-count, and extension constraints.</small>
        {systemError && <span role="alert">{systemError}</span>}
        <Input
          ref={setSystemInput}
          hidden
          type="file"
          accept={target === "folders" ? undefined : accept || undefined}
          multiple={Boolean(request.multiple) || target === "folders"}
          onChange={(event) => {
            systemSelected(Array.from(event.target.files ?? []));
            event.target.value = "";
          }}
        />
      </footer>}
    </div>
  </div>;
}

function normalizeHostedPickerRequest(value: unknown): HostedPickerRequest | null {
	if (!value || typeof value !== "object" || Array.isArray(value)) return null;
	const incoming = value as HostedPickerRequest;
	if (
		typeof incoming.onSelect !== "function" ||
		typeof incoming.onCancel !== "function"
	)
		return null;
	return {
		...incoming,
		onSystemSelect:
			typeof incoming.onSystemSelect === "function"
				? incoming.onSystemSelect
				: () => incoming.onCancel(),
	};
}
