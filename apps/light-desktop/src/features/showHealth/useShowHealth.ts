import { useEffect, useState } from "react";
import type { ShowHealthSnapshot } from "./contracts";
import { useActiveShowId } from "../deskSnapshot/DeskSnapshotState";
import { useDmxDiagnostics } from "../dmxDiagnostics/DmxDiagnosticsContext";

export function useShowHealth() {
	const read = useDmxDiagnostics()?.readShowHealth;
	const showId = useActiveShowId();
	const [snapshot, setSnapshot] = useState<ShowHealthSnapshot | null>(null);
	const [loading, setLoading] = useState(false);
	const [error, setError] = useState<string | null>(null);
	const [revision, refresh] = useState(0);
	useEffect(() => {
		let cancelled = false;
		setSnapshot(null);
		setError(null);
		setLoading(false);
		if (!showId) return;
		setLoading(true);
		void Promise.resolve()
			.then(() => {
				if (!read)
					throw new Error("Show Health is unavailable on this connection");
				return read();
			})
			.then((value) => {
				if (!cancelled && value.show_id === showId) setSnapshot(value);
			})
			.catch((reason: unknown) => {
				if (!cancelled)
					setError(reason instanceof Error ? reason.message : String(reason));
			})
			.finally(() => {
				if (!cancelled) setLoading(false);
			});
		return () => {
			cancelled = true;
		};
	}, [read, showId, revision]);
	return {
		showId,
		snapshot,
		loading,
		error,
		refresh: () => refresh((value) => value + 1),
	};
}

export function healthCounts(snapshot: ShowHealthSnapshot) {
	return (["error", "warning", "info"] as const)
		.map(
			(severity) =>
				`${snapshot.findings.filter((finding) => finding.severity === severity).length} ${severity === "info" ? "notes" : `${severity}s`}`,
		)
		.join(" · ");
}
