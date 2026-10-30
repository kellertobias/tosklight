import { Button, ModalPortal, ModalTitleBar } from "@tosklight/ui";
import { useEffect, useState } from "react";
import { useDmxDiagnostics } from "../features/dmxDiagnostics/DmxDiagnosticsContext";
import type { FixtureValueSourcesSnapshot } from "../api/generated/light-wire";
import { useActiveShowId } from "../features/deskSnapshot/DeskSnapshotState";
import "./fixtureValueSources.css";

type SourceTargets = readonly {
	fixtureId: string;
	id: string | number;
	name: string;
}[];

export function FixtureSheetSources({
	active,
	targets,
	selected,
}: {
	active: boolean;
	targets: SourceTargets;
	selected: ReadonlySet<string>;
}) {
	const [open, setOpen] = useState(false);
	useEffect(() => {
		if (!active) setOpen(false);
	}, [active]);
	return (
		<>
			<div className="fixture-source-toolbar">
				<Button
					disabled={!active || !targets.length}
					onClick={() => setOpen(true)}
				>
					Value sources
				</Button>
			</div>
			{open && active && (
				<FixtureValueSourcesDialog
					targets={targets}
					initialFixtureId={
						targets.find((target) => selected.has(target.fixtureId))?.fixtureId
					}
					onClose={() => setOpen(false)}
				/>
			)}
		</>
	);
}

export function FixtureValueSourcesDialog({
	targets,
	initialFixtureId,
	onClose,
}: {
	targets: SourceTargets;
	initialFixtureId?: string;
	onClose: () => void;
}) {
	const readSources = useDmxDiagnostics()?.readFixtureValueSources;
	const showId = useActiveShowId();
	const [chosenFixtureId, setFixtureId] = useState(
		initialFixtureId ?? targets[0]?.fixtureId ?? "",
	);
	const fixtureId = targets.some(
		(target) => target.fixtureId === chosenFixtureId,
	)
		? chosenFixtureId
		: (targets[0]?.fixtureId ?? "");
	const [snapshot, setSnapshot] = useState<FixtureValueSourcesSnapshot | null>(
		null,
	);
	const [error, setError] = useState<string | null>(null);
	const [loading, setLoading] = useState(false);
	const [refresh, setRefresh] = useState(0);
	useEffect(() => {
		let cancelled = false;
		setSnapshot(null);
		setError(null);
		setLoading(false);
		if (!fixtureId || !showId) return;
		setLoading(true);
		void Promise.resolve()
			.then(() => {
				if (!readSources)
					throw new Error(
						"Source inspection is unavailable on this connection",
					);
				return readSources(fixtureId);
			})
			.then((value) => {
				if (!cancelled) setSnapshot(value);
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
	}, [readSources, fixtureId, showId, refresh]);
	return (
		<ModalPortal onClose={onClose}>
			<div
				className="modal-backdrop"
				onPointerDown={(event) =>
					event.target === event.currentTarget && onClose()
				}
			>
				<section
					className="modal-card fixture-value-sources"
					role="dialog"
					aria-modal="true"
					aria-label="Fixture value sources"
				>
					<ModalTitleBar title="Fixture value sources" onClose={onClose} />
					<div className="fixture-source-controls">
						<label>
							Fixture{" "}
							<select
								aria-label="Inspect fixture"
								value={fixtureId}
								onChange={(event) => setFixtureId(event.target.value)}
							>
								{targets.map((target) => (
									<option key={target.fixtureId} value={target.fixtureId}>
										{target.id} · {target.name}
									</option>
								))}
							</select>
						</label>
						<Button
							disabled={loading || !fixtureId}
							onClick={() => setRefresh((value) => value + 1)}
						>
							Refresh sources
						</Button>
					</div>
					<p>
						Ordinary bases before Dynamic modulation and output masters. This is
						a captured inspection; refresh after changing the look.
					</p>
					{loading && <p role="status">Reading fixture sources…</p>}
					{!fixtureId && <p role="status">No fixtures available to inspect.</p>}
					{error && (
						<p role="alert">
							Cannot read sources: {error}. Check the connection and refresh.
						</p>
					)}
					{snapshot && <SourceSnapshot snapshot={snapshot} />}
				</section>
			</div>
		</ModalPortal>
	);
}

function sourceValueLabel(
	value: FixtureValueSourcesSnapshot["values"][number]["value"],
): string {
	switch (value.kind) {
		case "normalized":
			return `${Math.round(value.value * 100)}%`;
		case "discrete":
			return value.value;
		case "color_program":
			return value.value.kind === "semantic" ? "Color intent" : "Direct color";
		case "position":
			return value.value.kind === "angles"
				? "Position angles"
				: "Position target";
		case "zoom":
			return value.value.opening_degrees.kind === "value"
				? `${value.value.opening_degrees.value}°`
				: "Zoom spread";
		case "color_xyz":
			return "Color XYZ";
		case "spread":
			return "Spread";
		case "group_family":
			return "Group values";
		case "raw_dmx":
		case "raw_dmx_exact":
			return "Raw DMX (inspect DMX output)";
	}
}

function SourceSnapshot({
	snapshot,
}: {
	snapshot: FixtureValueSourcesSnapshot;
}) {
	return (
		<>
			<p role="status">
				Captured {new Date(snapshot.generated_at).toLocaleTimeString()} · Grand
				Master {Math.round(snapshot.grand_master * 100)}% · Blackout{" "}
				{snapshot.blackout ? "ON" : "off"} · Highlight{" "}
				{snapshot.highlighted ? "ON (output override)" : "off"}
			</p>
			<div className="fixture-source-table">
				<table>
					<thead>
						<tr>
							<th>Attribute</th>
							<th>Ordinary value</th>
							<th>Winning source</th>
						</tr>
					</thead>
					<tbody>
						{snapshot.values.map((entry) => (
							<tr key={entry.attribute}>
								<th>{entry.attribute}</th>
								<td>{sourceValueLabel(entry.value)}</td>
								<td>
									{entry.sources.map((source) => (
										<div key={source}>{source}</div>
									))}
								</td>
							</tr>
						))}
					</tbody>
				</table>
			</div>
			<p>
				Attributes without an ordinary contribution use their profile defaults.
				Freeze is listed as a held output; full Freeze bypasses masters and
				Highlight. Group masters, Highlight and profile mapping can further
				change other output; inspect DMX for channel values.
			</p>
			{snapshot.dynamics.length > 0 && (
				<>
					<h3>Dynamic and fixed overrides</h3>
					<ul>
						{snapshot.dynamics.map((entry, index) => (
							<li key={`${index}:${entry}`}>{entry}</li>
						))}
					</ul>
				</>
			)}
		</>
	);
}
