import { planPageStream } from "./printPlanPage";
import { mark, n, pdfDocument, saved, text } from "./printPdfOps";
import {
	BORDER,
	PRINT_BORDER_MM,
	PRINT_TITLE_HEIGHT_MM,
	PRINT_TITLE_WIDTH_MM,
	PT_MM,
	type CadPrintDocumentInfo,
} from "./printLayout";
import {
	type CadPrintPage,
	type CadSceneSnapshot,
	printPaperSize,
} from "./types";
import type { CadUnderlay } from "./underlays";

export {
	PRINT_BORDER_MM,
	PRINT_TITLE_HEIGHT_MM,
	PRINT_TITLE_WIDTH_MM,
	printGridMillimetres,
	printPageLayout,
	printScaleDenominator,
	rotatePrintPage,
} from "./printLayout";
export type { CadPrintDocumentInfo } from "./printLayout";

/** The printed document: one stream per included page, plan pages and fixture lists alike. */
export function buildCadPdf(
	scene: CadSceneSnapshot,
	pages: readonly CadPrintPage[],
	info: CadPrintDocumentInfo = {
		showName: "",
		lightingDesigner: "",
		showVersion: "",
		venue: "",
		contactEmail: "",
		contactPhone: "",
		project: "",
		showDate: "",
		lastSavedAt: 0,
		fixtureCount: scene.entities.length,
		universeCount: 0,
	},
	underlays: readonly CadUnderlay[] = [],
): Uint8Array {
	const drawings = new Map(
		scene.drawings.map((drawing) => [drawing.id, drawing]),
	);
	return pdfDocument(
		pages.flatMap((page) =>
			page.kind === "fixture_list"
				? fixtureListStreams(scene, page, info)
				: planPageStream(scene, drawings, page, info, underlays),
		),
	);
}


interface FixtureListRow {
	fixture: string;
	mode: string;
	fixtureId: string;
	patch: string;
	note: string;
}

function fixtureListRows(scene: CadSceneSnapshot): FixtureListRow[] {
	const grouped = new Map<string, typeof scene.entities>();
	for (const entity of scene.entities) {
		const current = grouped.get(entity.logicalFixtureId) ?? [];
		current.push(entity);
		grouped.set(entity.logicalFixtureId, current);
	}
	return [...grouped.values()]
		.map((entities) => {
			const first = entities[0];
			const patches = [...new Set(entities.map((entity) => entity.dmxAddress))];
			return {
				fixture: first.fixtureProfile || first.name,
				mode: first.mode || "-",
				fixtureId: first.fixtureDisplayId,
				patch: patches.join(" / ") || "Unpatched",
				note: first.note || "",
			};
		})
		.sort((left, right) => fixtureIdOrder(left.fixtureId, right.fixtureId));
}

function fixtureListStreams(
	scene: CadSceneSnapshot,
	page: CadPrintPage,
	info: CadPrintDocumentInfo,
) {
	const paper = printPaperSize(page);
	const W = paper.width * PT_MM;
	const H = paper.height * PT_MM;
	const columns = [
		{ title: "Fixture", width: 58 },
		{ title: "Mode", width: 48 },
		{ title: "Fixture ID", width: 24 },
		{ title: "DMX Patch", width: 35 },
		{ title: "Note", width: paper.width - PRINT_BORDER_MM * 2 - 165 },
	];
	const rows = fixtureListRows(scene);
	const rowHeight = 8 * PT_MM;
	const headerHeight = 9 * PT_MM;
	const top = H - BORDER - 24 * PT_MM;
	const bottom = BORDER + 18 * PT_MM;
	const rowsPerPage = Math.max(
		1,
		Math.floor((top - bottom - headerHeight) / rowHeight),
	);
	const chunks = rows.length
		? Array.from({ length: Math.ceil(rows.length / rowsPerPage) }, (_, index) =>
				rows.slice(index * rowsPerPage, (index + 1) * rowsPerPage),
			)
		: [[]];
	return chunks.map((chunk, pageIndex) => {
		const commands = [
			"1 1 1 rg",
			`0 0 ${n(W)} ${n(H)} re f`,
			"0.08 G",
			"0.7 w",
			`${n(BORDER)} ${n(BORDER)} ${n(W - BORDER * 2)} ${n(H - BORDER * 2)} re S`,
			...mark(BORDER + 4, H - BORDER - 54),
			text("ToskLight Architect", BORDER + 64, H - BORDER - 18, 13, true),
			text(
				`${info.project || info.showName || "Show"} - Fixture List${chunks.length > 1 ? ` ${pageIndex + 1}/${chunks.length}` : ""}`,
				BORDER + 64,
				H - BORDER - 35,
				10,
				true,
			),
			text(
				[info.venue, info.lightingDesigner, info.showDate]
					.filter(Boolean)
					.join(" / "),
				BORDER + 64,
				H - BORDER - 49,
				7,
			),
		];
		let x = BORDER;
		let y = top;
		commands.push(
			"0.93 0.96 0.98 rg",
			`${n(x)} ${n(y - headerHeight)} ${n(W - BORDER * 2)} ${n(headerHeight)} re f`,
		);
		for (const column of columns) {
			commands.push(
				"0.08 G",
				`${n(x)} ${n(y - headerHeight)} ${n(column.width * PT_MM)} ${n(headerHeight)} re S`,
				text(column.title, x + 4, y - headerHeight + 8, 7, true),
			);
			x += column.width * PT_MM;
		}
		y -= headerHeight;
		for (const row of chunk) {
			x = BORDER;
			const values = [
				row.fixture,
				row.mode,
				row.fixtureId,
				row.patch,
				row.note,
			];
			for (let index = 0; index < columns.length; index++) {
				const width = columns[index].width * PT_MM;
				commands.push(
					"0.55 G",
					`${n(x)} ${n(y - rowHeight)} ${n(width)} ${n(rowHeight)} re S`,
					text(
						ellipsize(
							values[index],
							Math.max(4, Math.floor(columns[index].width / 2.4)),
						),
						x + 4,
						y - rowHeight + 8,
						6.5,
					),
				);
				x += width;
			}
			y -= rowHeight;
		}
		commands.push(
			text(`Show  ${info.showName || "-"}`, BORDER + 4, BORDER + 13, 6.5),
			text(
				`Version  ${info.showVersion || "-"}`,
				BORDER + 130,
				BORDER + 13,
				6.5,
			),
			text(
				`Saved  ${saved(info.lastSavedAt)}`,
				W - BORDER - 180,
				BORDER + 13,
				6.5,
			),
		);
		return { content: commands.join("\n"), width: W, height: H };
	});
}

function fixtureIdOrder(left: string, right: string) {
	const numeric = (value: string) => value.split(".").map(Number);
	const a = numeric(left);
	const b = numeric(right);
	for (let index = 0; index < Math.max(a.length, b.length); index++) {
		const difference = (a[index] ?? 0) - (b[index] ?? 0);
		if (difference) return difference;
	}
	return left.localeCompare(right);
}

function ellipsize(value: string, length: number) {
	const clean = value.replace(/\s+/g, " ").trim() || "-";
	return clean.length <= length
		? clean
		: `${clean.slice(0, Math.max(1, length - 3))}...`;
}

