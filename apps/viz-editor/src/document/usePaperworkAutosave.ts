import { useEffect, useRef, useState } from "react";
import type { CadPaperwork } from "../cad/CadProjectPanel";
import { documentSession, type DocumentSummary } from "./session";
import { registerPendingDocumentSave } from "./pendingDocumentSaves";

export function paperworkOf(document: DocumentSummary): CadPaperwork {
	return {
		project: document.project ?? "", lightingDesigner: document.lightingDesigner ?? "",
		venue: document.venue ?? "", contactEmail: document.contactEmail ?? "",
		contactPhone: document.contactPhone ?? "", showDate: document.showDate ?? "",
		showVersion: document.showVersion ?? "", companyLogo: document.companyLogo ?? "",
	};
}

/** Serialize the form's writes and retain a newer edit while the preceding save completes. */
export function usePaperworkAutosave(document: DocumentSummary, onDocument: (document: DocumentSummary) => void) {
	const [draft, setDraft] = useState(() => paperworkOf(document));
	const [saving, setSaving] = useState(false);
	const [error, setError] = useState<string | null>(null);
	const current = useRef(draft);
	const dirty = useRef(false);
	const callback = useRef(onDocument);
	callback.current = onDocument;
	const writer = useRef<{ update(value: CadPaperwork): void; retry(): void } | null>(null);

	useEffect(() => {
		let disposed = false;
		let timer: ReturnType<typeof setTimeout> | undefined;
		let pending: CadPaperwork | undefined;
		let flight: Promise<void> | undefined;
		let failure: unknown;
		const showId = document.showId;
		const initial = paperworkOf(document);
		current.current = initial;
		dirty.current = false;
		setDraft(initial);
		setError(null);
		setSaving(false);
		async function drain() {
			try {
				while (pending) {
					const next = pending;
					pending = undefined;
					try {
						const saved = await documentSession.savePaperwork(next, showId);
						if (!disposed) {
							dirty.current = pending !== undefined;
							setError(null);
							callback.current(saved);
						}
					} catch (reason) {
						pending ??= next;
						failure = reason;
						if (!disposed) setError(`Project information could not be saved: ${String(reason)}`);
						throw reason;
					}
				}
			} finally {
				if (!disposed) setSaving(false);
			}
		}
		function flush(): Promise<void> {
			clearTimeout(timer);
			if (failure !== undefined) return Promise.reject(failure);
			if (flight) return flight;
			if (!pending) return Promise.resolve();
			flight = drain().finally(() => { flight = undefined; });
			return flight;
		}
		function start() { void flush().catch(() => { /* The form exposes the error and Retry. */ }); }
		const unregister = registerPendingDocumentSave(flush);
		writer.current = {
			update(value) {
				pending = value;
				dirty.current = true;
				if (failure !== undefined) return;
				setSaving(true);
				clearTimeout(timer);
				timer = setTimeout(start, 350);
			},
			retry() {
				failure = undefined;
				setError(null);
				setSaving(true);
				clearTimeout(timer);
				start();
			},
		};
		return () => {
			disposed = true;
			writer.current = null;
			clearTimeout(timer);
			void flush().catch(() => {}).finally(unregister);
		};
	}, [document.showId]);

	useEffect(() => {
		if (!dirty.current) {
			const next = paperworkOf(document);
			current.current = next;
			setDraft(next);
		}
	}, [document]);

	return {
		draft, saving, error,
		change(field: keyof CadPaperwork, value: string) {
			const next = { ...current.current, [field]: value };
			current.current = next;
			setDraft(next);
			writer.current?.update(next);
		},
		retry: () => writer.current?.retry(),
	};
}
