import { Button } from "@tosklight/ui";
import { useState } from "react";
import type { ExportedMvrFile } from "../../api/client/shows";

/** One finished MVR export as the server reported it, and where the operator saved it. */
export interface MvrExportReport extends ExportedMvrFile {
	/** The destination the archive went to: a desk or drive label. */
	location: string;
}

/**
 * The server's summary of an archive Save As just wrote. It stays until the operator dismisses
 * it or starts another save or export, so no export warning disappears unread.
 */
export function MvrExportSummary({ report, onDismiss }: { report: MvrExportReport; onDismiss: () => void }) {
	const { summary } = report;
	const [copy, setCopy] = useState<"idle" | "copied" | "failed">("idle");
	const warnings = summary.warnings;
	async function copyWarnings() {
		try {
			await navigator.clipboard.writeText(warnings.join("\n"));
			setCopy("copied");
		} catch {
			setCopy("failed");
		}
	}
	return <section className="mvr-summary mvr-export-summary" role="status" aria-label="MVR export summary">
		<b>Exported MVR to {report.location} / {report.path}</b>
		<p>{summary.fixtures} fixtures · {summary.scenery} scenery objects</p>
		<p>Not included: {summary.omitted.join(", ")}</p>
		{warnings.length > 0 && <>
			<p className="modal-warning">{warnings.length === 1 ? "1 export warning" : `${warnings.length} export warnings`} — check the archive in the receiving application:</p>
			<ul className="mvr-export-warnings" aria-label="MVR export warnings">
				{warnings.map((warning, index) => <li className="modal-warning" key={`${index}:${warning}`}>{warning}</li>)}
			</ul>
		</>}
		<div className="mvr-export-summary-actions">
			{warnings.length > 0 && <Button onClick={() => void copyWarnings()}>
				{copy === "copied" ? "Warnings copied" : copy === "failed" ? "Copy failed; retry" : "Copy warnings"}
			</Button>}
			<Button onClick={onDismiss}>Dismiss</Button>
		</div>
	</section>;
}
