import { formatErrorDetails } from "@tosklight/ui";
import { save } from "@tauri-apps/plugin-dialog";
import { useRef, useState } from "react";
import { afterPendingDocumentSaves } from "../document/pendingDocumentSaves";
import { buildCadPdf } from "./print";
import { loadPrintFonts } from "./printFonts";
import { cadSession } from "./session";

type PdfArguments = Parameters<typeof buildCadPdf>;
interface ExportOptions {
	scene: PdfArguments[0] | null;
	pages: PdfArguments[1];
	loadInfo(): Promise<PdfArguments[2]>;
	underlays: PdfArguments[3];
	annotations: NonNullable<PdfArguments[4]>;
	blocked: boolean;
	onNotice(message: string): void;
	onError(message: string): void;
}

/** Keep the native save dialog and PDF export lifecycle together. */
export function useCadPdfExport(options: ExportOptions) {
	const [exporting, setExporting] = useState(false);
	const active = useRef(false);
	async function exportPdf() {
		const { scene, pages, loadInfo, underlays, annotations, blocked, onNotice, onError } = options;
		if (!scene || blocked || active.current) return;
		const selected = pages.filter((page) => page.included);
		if (!selected.length) return;
		active.current = true;
		setExporting(true);
		try {
			await afterPendingDocumentSaves(async () => {
				const path = await save({
					title: "Export CAD plan pages",
					defaultPath: "ToskLight Architect Plan.pdf",
					filters: [{ name: "PDF document", extensions: ["pdf"] }],
				});
				if (!path) return;
				const info = await loadInfo();
				const pdfPath = path.toLowerCase().endsWith(".pdf") ? path : `${path}.pdf`;
				await cadSession.exportPdf(pdfPath, buildCadPdf(
					scene, selected, info, underlays, annotations, await loadPrintFonts(annotations),
				));
				onNotice(`Exported ${selected.length} plan pages.`);
			});
		} catch (reason) {
			onError(formatErrorDetails(reason));
		} finally {
			active.current = false;
			setExporting(false);
		}
	}
	return { exporting, exportPdf };
}
