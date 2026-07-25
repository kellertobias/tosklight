import { useCallback, useEffect } from "react";
import type { TextDocument } from "../../api/types";
import { friendlyError, isMissingError, isSameDocumentVersion } from "./files";
import type { TextEditorState } from "./state";

export function useDocumentAcceptance(model: TextEditorState) {
	const {
		availabilityRef,
		dirtyRef,
		documentRef,
		externalDocumentRef,
		setAvailability,
		setDirty,
		setDocument,
		setExternalDocument,
		setNotice,
		setText,
		textRef,
	} = model;
	const acceptDocument = useCallback(
		(next: TextDocument, message?: string) => {
			documentRef.current = next;
			externalDocumentRef.current = null;
			dirtyRef.current = false;
			textRef.current = next.text;
			setDocument(next);
			setExternalDocument(null);
			setText(next.text);
			setDirty(false);
			availabilityRef.current = "ready";
			setAvailability("ready");
			setNotice(
				next.read_only
					? {
							kind: "info",
							text: "This file is read-only. Its contents can be copied with Save As, but the original cannot be changed.",
						}
					: message
						? { kind: "info", text: message }
						: null,
			);
		},
		[
			availabilityRef,
			dirtyRef,
			documentRef,
			externalDocumentRef,
			setAvailability,
			setDirty,
			setDocument,
			setExternalDocument,
			setNotice,
			setText,
			textRef,
		],
	);
	const surfaceExternalDocument = useCallback(
		(next: TextDocument, source: string) => {
			const current = documentRef.current;
			if (
				dirtyRef.current &&
				!isSameDocumentVersion(current, next) &&
				textRef.current !== next.text
			) {
				externalDocumentRef.current = next;
				setExternalDocument(next);
				availabilityRef.current = "ready";
				setAvailability("ready");
				setNotice({
					kind: "conflict",
					text: `${source} changed this file while you have unsaved edits. Your text is preserved; compare, reload, or save your version as a new file.`,
				});
				return;
			}
			acceptDocument(
				next,
				`${source} saved a newer version. The editor has been updated.`,
			);
		},
		[
			acceptDocument,
			availabilityRef,
			dirtyRef,
			documentRef,
			externalDocumentRef,
			setAvailability,
			setExternalDocument,
			setNotice,
			textRef,
		],
	);
	return { acceptDocument, surfaceExternalDocument };
}

export function useSelectedTextDocument(
	model: TextEditorState,
	acceptDocument: (next: TextDocument, message?: string) => void,
) {
	const {
		availabilityRef,
		dirtyRef,
		documentRef,
		externalDocumentRef,
		relocatedAssociation,
		selectedPath,
		selectedRoot,
		serverRef,
		setAvailability,
		setDirty,
		setDocument,
		setExternalDocument,
		setNotice,
		setText,
		textRef,
	} = model;
	const clearDocument = useCallback(() => {
		documentRef.current = null;
		externalDocumentRef.current = null;
		dirtyRef.current = false;
		textRef.current = "";
		setDocument(null);
		setExternalDocument(null);
		setText("");
		setDirty(false);
	}, [
		dirtyRef,
		documentRef,
		externalDocumentRef,
		setDirty,
		setDocument,
		setExternalDocument,
		setText,
		textRef,
	]);
	useEffect(() => {
		let cancelled = false;
		const relocated = relocatedAssociation.current;
		if (relocated?.root === selectedRoot && relocated.path === selectedPath) {
			relocatedAssociation.current = null;
			return;
		}
		if (!selectedRoot || !selectedPath) {
			clearDocument();
			availabilityRef.current = "none";
			setAvailability("none");
			setNotice(null);
			return;
		}
		availabilityRef.current = "loading";
		setAvailability("loading");
		setNotice({
			kind: "info",
			text: `Opening ${selectedPath}…`,
		});
		void serverRef.current
			.readTextFile(selectedRoot, selectedPath)
			.then((next) => {
				if (!cancelled) acceptDocument(next);
			})
			.catch((error) => {
				if (cancelled) return;
				clearDocument();
				const availability = isMissingError(error) ? "missing" : "none";
				availabilityRef.current = availability;
				setAvailability(availability);
				setNotice({
					kind: "error",
					text: isMissingError(error)
						? `The selected file is missing, moved, deleted, or its location is unavailable: ${selectedPath}`
						: `Could not open ${selectedPath}: ${friendlyError(error)}`,
				});
			});
		return () => {
			cancelled = true;
		};
	}, [
		acceptDocument,
		availabilityRef,
		clearDocument,
		relocatedAssociation,
		selectedPath,
		selectedRoot,
		serverRef,
		setAvailability,
		setNotice,
	]);
}

export type DocumentAcceptance = ReturnType<typeof useDocumentAcceptance>;
