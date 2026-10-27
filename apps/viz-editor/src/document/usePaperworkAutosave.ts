import { useEffect, useRef, useState } from "react";
import type { CadPaperwork } from "../cad/CadProjectPanel";
import { documentSession, type DocumentSummary } from "./session";

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
		let active = false;
		let failed = false;
		const showId = document.showId;
		const initial = paperworkOf(document);
		current.current = initial;
		dirty.current = false;
		setDraft(initial);
		setError(null);
		setSaving(false);
		async function flush() {
			if (active || !pending || failed) return;
			active = true;
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
				failed = true;
				if (!disposed) setError(`Project information could not be saved: ${String(reason)}`);
			} finally {
				active = false;
				if (pending && !failed) void flush();
				else if (!disposed) setSaving(false);
			}
		}
		writer.current = {
			update(value) {
				pending = value;
				dirty.current = true;
				if (failed) return;
				setSaving(true);
				clearTimeout(timer);
				timer = setTimeout(() => void flush(), 350);
			},
			retry() {
				failed = false;
				setError(null);
				setSaving(true);
				clearTimeout(timer);
				void flush();
			},
		};
		return () => {
			disposed = true;
			writer.current = null;
			clearTimeout(timer);
			void flush();
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
