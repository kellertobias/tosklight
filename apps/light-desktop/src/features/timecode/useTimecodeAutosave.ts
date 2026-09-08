import { useCallback, useEffect, useMemo, useState } from "react";
import type {
	TimecodeDefinition,
	TimecodeObjectRecord,
} from "../../api/types/timecode";
import {
	TimecodeAutosaveWriter,
	type TimecodeAutosaveApi,
} from "./TimecodeAutosaveWriter";

/** Owns serialized immediate saves and retains the last authoritative audio identity. */
export function useTimecodeAutosave({
	showId,
	item,
	draft,
	api,
}: {
	showId: string | null;
	item: TimecodeObjectRecord & { isNew?: true };
	draft: TimecodeDefinition;
	api: TimecodeAutosaveApi;
}) {
	const initialRecord = "isNew" in item ? null : item;
	const writer = useMemo(
		() =>
			showId ? new TimecodeAutosaveWriter(showId, initialRecord, api) : null,
		[api, item, showId],
	);
	const [record, setRecord] = useState<TimecodeObjectRecord | null>(
		initialRecord,
	);
	const [saving, setSaving] = useState(Boolean(writer && !initialRecord));
	const [saveError, setSaveError] = useState<string | null>(null);
	const [saveAttempt, setSaveAttempt] = useState(0);
	useEffect(() => {
		if (!writer) return;
		let current = true;
		setSaving(true);
		void writer
			.enqueue(draft)
			.then((saved) => {
				if (!current) return;
				setRecord(saved);
				setSaveError(null);
			})
			.catch((reason) => {
				if (current)
					setSaveError(
						`Autosave failed: ${reason instanceof Error ? reason.message : String(reason)}`,
					);
			})
			.finally(() => current && setSaving(false));
		return () => {
			current = false;
		};
	}, [draft, writer, saveAttempt]);
	const retry = useCallback(() => setSaveAttempt((value) => value + 1), []);
	const flush = useCallback(async () => writer?.flush(), [writer]);
	return { record, saving, saveError, retry, flush };
}
