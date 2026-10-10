import { Button, type ButtonProps, ErrorAlert } from "@tosklight/ui";
import {
	type RefObject,
	useCallback,
	useEffect,
	useRef,
	useState,
} from "react";
import { useFiles } from "../../features/files/FilesContext";
import { openFileManagerPicker } from "../../windows/FileManagerPickerHost";

export interface RootConfinedFilePickerButtonProps {
	label: string;
	allowedExtensions?: string[];
	multiple?: boolean;
	disabled?: boolean;
	buttonClassName?: string;
	variant?: ButtonProps["variant"];
	hideButton?: boolean;
	triggerRef?: RefObject<(() => void) | null>;
	onReadBusyChange?: (busy: boolean) => void;
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
	onReadBusyChange,
}: RootConfinedFilePickerButtonProps) {
	const server = useFiles();
	const [busy, setBusy] = useState(false);
	const [error, setError] = useState("");

	const locked = useRef(false);
	const mounted = useRef(true);
	useEffect(() => {
		mounted.current = true;
		return () => {
			mounted.current = false;
		};
	}, []);
	const choose = useCallback(async () => {
		if (disabled || locked.current) return;
		locked.current = true;
		setError("");
		let reading = false;
		try {
			const result = await openFileManagerPicker({
				purpose: label,
				target: "files",
				multiple,
				allowedExtensions,
			});
			if (!result || !mounted.current) return;
			setBusy(true);
			reading = true;
			onReadBusyChange?.(true);
			const files = Array.isArray(result)
				? await Promise.all(
						result.map(async ({ rootId, entry }) => {
							const content = await server.fileContent(rootId, entry.path);
							return new File([content], entry.name, {
								type: content.type,
								lastModified: entry.modified_millis ?? Date.now(),
							});
						}),
					)
				: result.files;
			if (!mounted.current) return;
			reading = false;
			onReadBusyChange?.(false);
			await onFiles(files);
		} catch (reason) {
			if (mounted.current)
				setError(`Could not use the selected file: ${String(reason)}`);
		} finally {
			locked.current = false;
			if (mounted.current) {
				if (reading) onReadBusyChange?.(false);
				setBusy(false);
			}
		}
	}, [
		allowedExtensions,
		disabled,
		label,
		multiple,
		onFiles,
		onReadBusyChange,
		server,
	]);

	useEffect(() => {
		if (!triggerRef) return;
		triggerRef.current = () => void choose();
		return () => {
			triggerRef.current = null;
		};
	}, [choose, triggerRef]);

	return (
		<span className="root-confined-file-picker">
			{!hideButton && (
				<Button
					aria-label={label}
					variant={variant}
					className={buttonClassName}
					disabled={disabled || busy}
					onClick={() => void choose()}
				>
					{busy ? "Loading selected file…" : label}
				</Button>
			)}
			{error && (
				<ErrorAlert as="small" role="alert">
					{error}
				</ErrorAlert>
			)}
		</span>
	);
}
