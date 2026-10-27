import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { CadPrintPage } from "./types";

export interface PrintPageSnapshot {
	showId: string;
	revision: number;
	pages: CadPrintPage[] | null;
}

/** Revision-guarded object edits and authoritative changes from every Architect window. */
export const printPageSession = {
	all: (showId: string) => invoke<PrintPageSnapshot>("cad_print_pages", { showId }),
	save: (showId: string, pages: CadPrintPage[], onlyIfAbsent: boolean, expectedRevision: number) =>
		invoke<PrintPageSnapshot>("save_cad_print_pages", {
			showId, pages, onlyIfAbsent, expectedRevision,
		}),
	onDelta: (handler: (snapshot: PrintPageSnapshot) => void): Promise<UnlistenFn> =>
		listen<PrintPageSnapshot>("cad-print-pages-delta", (event) => handler(event.payload)),
};
