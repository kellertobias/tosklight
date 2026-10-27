import { formatErrorDetails } from "@tosklight/ui";
import { save } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import { buildCadPdf } from "./print";
import { loadPrintFonts } from "./printFonts";
import { cadSession } from "./session";

type PdfArguments = Parameters<typeof buildCadPdf>;
interface ExportOptions {
	scene: PdfArguments[0] | null;
	pages: PdfArguments[1];
	info: PdfArguments[2];
	underlays: PdfArguments[3];
	annotations: NonNullable<PdfArguments[4]>;
	blocked: boolean;
	onNotice(message: string): void;
	onError(message: string): void;
}

/** Keep the native save dialog and PDF export lifecycle together. */
export function useCadPdfExport(options: ExportOptions) {
	const [exporting, setExporting] = useState(false);
	async function exportPdf() {
		const { scene, pages, info, underlays, annotations, blocked, onNotice, onError } = options;
		if (!scene || blocked) return;
		const selected = pages.filter((page) => page.included);
		if (!selected.length) return;
		const path = await save({
			title: "Export CAD plan pages",
			defaultPath: "ToskLight Architect Plan.pdf",
			filters: [{ name: "PDF document", extensions: ["pdf"] }],
		});
		if (!path) return;
		const pdfPath = path.toLowerCase().endsWith(".pdf") ? path : `${path}.pdf`;
		setExporting(true);
		try {
			await cadSession.exportPdf(pdfPath, buildCadPdf(
				scene, selected, info, underlays, annotations, await loadPrintFonts(annotations),
			));
			onNotice(`Exported ${selected.length} plan pages.`);
		} catch (reason) {
			onError(formatErrorDetails(reason));
		} finally {
			setExporting(false);
		}
	}
	return { exporting, exportPdf };
}
