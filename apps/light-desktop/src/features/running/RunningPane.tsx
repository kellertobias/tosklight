import { Button } from "@tosklight/ui";
import { WindowHeader, WindowScrollArea } from "@tosklight/ui/window-kit";
import { useMemo, useRef, useState } from "react";
import {
	filterRunningRows,
	type RunningFilter,
	type RunningKind,
	type RunningRow,
	runningKindLabel,
} from "./model";
import "../../windows/RunningWindow.css";

const FILTERS: readonly { id: RunningFilter; label: string }[] = [
	{ id: "all", label: "All" },
	{ id: "cue_list", label: "Cuelists" },
	{ id: "dynamic", label: "Dynamics" },
	{ id: "timecode", label: "Timecodes" },
	{ id: "macro", label: "Macros" },
];

export interface RunningPaneProps {
	rows: readonly RunningRow[];
	loading?: boolean;
	error?: string | null;
	compact?: boolean;
	initialFilter?: RunningFilter;
	filter?: RunningFilter;
	onFilterChange?(filter: RunningFilter): void;
}

export function RunningPane({
	rows,
	loading = false,
	error = null,
	compact = false,
	initialFilter = "all",
	filter: controlledFilter,
	onFilterChange,
}: RunningPaneProps) {
	const [localFilter, setLocalFilter] = useState<RunningFilter>(initialFilter);
	const [stopping, setStopping] = useState<ReadonlySet<string>>(
		() => new Set(),
	);
	const stoppingRef = useRef(new Set<string>());
	const [actionError, setActionError] = useState<string | null>(null);
	const filter = controlledFilter ?? localFilter;
	const visibleRows = useMemo(
		() => filterRunningRows(rows, filter),
		[filter, rows],
	);
	const setFilter = (next: RunningFilter) => {
		if (controlledFilter === undefined) setLocalFilter(next);
		onFilterChange?.(next);
	};
	const stop = async (row: RunningRow) => {
		if (stoppingRef.current.has(row.key)) return;
		stoppingRef.current.add(row.key);
		setActionError(null);
		setStopping((current) => new Set(current).add(row.key));
		try {
			await row.off();
		} catch (cause) {
			setActionError(cause instanceof Error ? cause.message : String(cause));
		} finally {
			stoppingRef.current.delete(row.key);
			setStopping((current) => {
				const next = new Set(current);
				next.delete(row.key);
				return next;
			});
		}
	};
	return (
		<section className="running-window">
			{!compact && (
				<WindowHeader
					title="Running"
					info={{ primary: `${visibleRows.length} running` }}
					actions={[
						FILTERS.map((item) => ({
							id: item.id,
							label: item.label,
							active: filter === item.id,
							onClick: () => setFilter(item.id),
						})),
					]}
				/>
			)}
			<WindowScrollArea>
				<div className="running-list" aria-live="polite">
					{visibleRows.map((row) => (
						<RunningRowView
							key={row.key}
							row={row}
							stopping={stopping.has(row.key)}
							onOff={() => void stop(row)}
						/>
					))}
					{visibleRows.length === 0 && (
						<p className="empty-window-message">
							{loading ? "Running objects loading…" : emptyMessage(filter)}
						</p>
					)}
					{(error || actionError) && (
						<p className="running-error" role="alert">
							Running state unavailable: {error ?? actionError}
						</p>
					)}
				</div>
			</WindowScrollArea>
		</section>
	);
}

function RunningRowView({
	row,
	stopping,
	onOff,
}: {
	row: RunningRow;
	stopping: boolean;
	onOff(): void;
}) {
	return (
		<article className="running-row" data-running-kind={row.kind}>
			<span>
				<b>{identityLabel(row)}</b>
				<small>
					{kindSingular(row.kind)} ·{" "}
					{row.cueNumber == null ? "Cue —" : `Cue ${row.cueNumber}`} ·{" "}
					{row.status}
				</small>
			</span>
			<Button
				className="danger"
				aria-label={`Turn off ${kindSingular(row.kind)} ${row.number ?? "without number"} ${row.name}`}
				disabled={stopping}
				onClick={onOff}
			>
				{stopping ? "Turning off…" : "Off"}
			</Button>
		</article>
	);
}

function identityLabel(row: RunningRow): string {
	return row.number == null ? row.name : `${row.number} · ${row.name}`;
}

function kindSingular(kind: RunningKind): string {
	return kind === "cue_list" ? "Cuelist" : runningKindLabel(kind).slice(0, -1);
}

function emptyMessage(filter: RunningFilter): string {
	return filter === "all"
		? "Nothing is running."
		: `No ${runningKindLabel(filter)} are running.`;
}
