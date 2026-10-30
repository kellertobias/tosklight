import { ModalFrame, SelectField } from "@tosklight/ui";
import { useEffect, useMemo, useState } from "react";
import { createPortal } from "react-dom";
import {
	buildPaperwork,
	PAPERWORK_REPORTS,
	type PatchedFixture,
	type PaperworkKind,
	type PaperworkReport,
	paperworkCsv,
	UNPATCHED,
} from "./paperwork";
import "./paperwork.css";

function fileName(report: PaperworkReport, show: string) {
	return `${show ? `${show} – ` : ""}${report.title}.csv`.replace(/[\\/:*?"<>|]/g, "-");
}

function download(name: string, text: string) {
	const url = URL.createObjectURL(new Blob([text], { type: "text/csv;charset=utf-8" }));
	const link = document.createElement("a");
	link.href = url;
	link.download = name;
	document.body.append(link);
	link.click();
	link.remove();
	setTimeout(() => URL.revokeObjectURL(url), 1000);
}

function ReportTable({ report }: { report: PaperworkReport }) {
	return (
		<table className="paperwork-table" aria-label={report.title}>
			<thead>
				<tr>
					{report.columns.map((column) => (
						<th key={column}>{column}</th>
					))}
				</tr>
			</thead>
			<tbody>
				{report.rows.map((row, index) => (
					// biome-ignore lint/suspicious/noArrayIndexKey: Report rows have no identity beyond their order.
					<tr key={index}>
						{row.map((cell, column) => (
							<td key={report.columns[column]} className={cell === UNPATCHED ? "is-unpatched" : undefined}>
								{cell}
							</td>
						))}
					</tr>
				))}
			</tbody>
		</table>
	);
}

/**
 * Lighting paperwork from the live patch: hookup, schedule, DMX sheet, pull lists and symbol key.
 *
 * It reads the same fixtures the sheet shows, so it is always current. CSV saves or copies the
 * report; Print hands only the report to the system print dialog, where Save as PDF makes the PDF.
 */
export function PaperworkDialog({
	fixtures,
	showName = "",
	onClose,
}: {
	fixtures: readonly PatchedFixture[];
	showName?: string;
	onClose: () => void;
}) {
	const [kind, setKind] = useState<PaperworkKind>("channel_hookup");
	const [status, setStatus] = useState("");
	const [printing, setPrinting] = useState(false);
	const report = useMemo(() => buildPaperwork(kind, fixtures), [fixtures, kind]);
	useEffect(() => {
		if (!printing) return;
		const done = () => setPrinting(false);
		window.addEventListener("afterprint", done);
		// Let the print copy mount before the dialog takes its snapshot of the page.
		const frame = requestAnimationFrame(() => window.print());
		return () => {
			cancelAnimationFrame(frame);
			window.removeEventListener("afterprint", done);
		};
	}, [printing]);
	const copy = async () => {
		try {
			await navigator.clipboard.writeText(paperworkCsv(report));
			setStatus(`Copied ${report.title} as CSV.`);
		} catch (reason) {
			setStatus(`Could not copy: ${String(reason)}`);
		}
	};
	return (
		<>
			<ModalFrame
				title="Paperwork"
				ariaLabel="Paperwork"
				dialogClassName="nested-modal paperwork-modal"
				closeLabel="Close Paperwork"
				onClose={onClose}
				groups={[
					{
						id: "paperwork-export",
						actions: [
							{ id: "copy-csv", label: "Copy CSV", disabled: !report.rows.length, onPress: () => void copy() },
							{
								id: "export-csv",
								label: "Export CSV",
								disabled: !report.rows.length,
								onPress: () => {
									download(fileName(report, showName), paperworkCsv(report));
									setStatus(`Exported ${fileName(report, showName)}.`);
								},
							},
							{ id: "print", label: "Print / PDF", disabled: !report.rows.length, onPress: () => setPrinting(true) },
						],
					},
				]}
			>
				<div className="paperwork-body">
					<div className="paperwork-toolbar">
						<SelectField
							label="Report"
							ariaLabel="Report"
							value={kind}
							options={PAPERWORK_REPORTS.map((item) => ({ value: item.kind, label: item.title }))}
							onChange={(value) => {
								setKind(value as PaperworkKind);
								setStatus("");
							}}
						/>
						<p>{PAPERWORK_REPORTS.find((item) => item.kind === kind)?.description} {report.rows.length} rows.</p>
					</div>
					{status && <p role="status" className="paperwork-status">{status}</p>}
					{report.rows.length ? (
						<div className="paperwork-scroll">
							<ReportTable report={report} />
						</div>
					) : (
						<p className="paperwork-empty">Nothing to list for this report yet.</p>
					)}
				</div>
			</ModalFrame>
			{printing &&
				createPortal(
					<div className="paperwork-print">
						<header>
							<h1>{report.title}</h1>
							<p>
								{showName}
								{showName ? " · " : ""}
								{new Date().toLocaleString()}
							</p>
						</header>
						<ReportTable report={report} />
					</div>,
					document.body,
				)}
		</>
	);
}
