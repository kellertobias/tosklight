import { useRef } from "react";
import { pageFrameGeometry, pageFrameDelta } from "./printFrameGeometry";
import architectIconUrl from "../../../../assets/branding/tosklight-icon-print-dark.svg";
import { companyLogoUrl, parseCompanyLogo } from "../document/companyLogo";
import { PrintPageLabels, PrintPageSettings } from "./CadPrintPageParts";
import {
	type CadPrintDocumentInfo,
	PRINT_TITLE_MARK,
	PRINT_TITLE_DIVIDER_X,
	printTitleSeparatorY,
	printTitleEntries,
	fitTitleText,
	PRINT_BORDER_MM,
	PRINT_TITLE_HEIGHT_MM,
	PRINT_TITLE_WIDTH_MM,
	printGridMillimetres,
	printScaleDenominator,
} from "./print";
import { TITLE_W, TITLE_H } from "./printLayout";
import type { CadEntity, CadPrintPage, TileCamera } from "./types";
import { printPageHeight, printPaperSize } from "./types";
import { placedPolylines, underlaysForPage } from "./underlayGeometry";
import type { CadUnderlay } from "./underlays";

interface PrintFrameProps {
	page: CadPrintPage;
	entities: readonly CadEntity[];
	underlays: readonly CadUnderlay[];
	camera: TileCamera;
	viewportRotation?: number;
	selected: boolean;
	onSelect(): void;
	onChange(change: Partial<CadPrintPage>): void;
	onCamera(camera: TileCamera): void;
	documentInfo?: CadPrintDocumentInfo;
}

type FrameGesture = "move" | "scale" | "pan";

/**
 * Dragging a page: the sheet moves the page, the corner handle scales it, and the middle button
 * pans the tile underneath instead, so a page can be pushed around without losing the rig.
 */
function usePrintFrameInteraction({
	page,
	camera,
	viewportRotation = 0,
	onChange,
	onCamera,
}: Pick<PrintFrameProps, "page" | "camera" | "viewportRotation" | "onChange" | "onCamera">) {
	const interaction = useRef<{
		type: FrameGesture;
		start: [number, number];
		centre: [number, number];
		width: number;
		camera: TileCamera;
	} | null>(null);

	function begin(gesture: FrameGesture, event: React.PointerEvent<HTMLElement>) {
		if (gesture === "scale" && event.button !== 1 && page.fixedScaleDenominator !== undefined) return;
		interaction.current = {
			type: event.button === 1 ? "pan" : gesture,
			start: [event.clientX, event.clientY],
			centre: page.centreMillimetres,
			width: page.widthMillimetres,
			camera,
		};
		event.currentTarget.setPointerCapture?.(event.pointerId);
	}

	function move(event: React.PointerEvent<HTMLElement>) {
		const active = interaction.current;
		if (!active) return;
		const dx = event.clientX - active.start[0];
		const dy = event.clientY - active.start[1];
		const delta = pageFrameDelta(page, viewportRotation, [dx / camera.zoom, -dy / camera.zoom]);
		if (active.type === "pan") {
			onCamera({
				...active.camera,
				pan: [
					active.camera.pan[0] + dx / active.camera.zoom,
					active.camera.pan[1] - dy / active.camera.zoom,
				],
			});
		} else if (active.type === "move") {
			onChange({
				centreMillimetres: [
					active.centre[0] + delta[0],
					active.centre[1] + delta[1],
				],
			});
		} else {
			onChange({
				widthMillimetres: Math.max(
					500,
					active.width +
						Math.max(
							delta[0] * 2,
							(-delta[1] * 2 * page.widthMillimetres) / printPageHeight(page),
						),
				),
			});
		}
	}

	function stop(event: React.PointerEvent<HTMLElement>) {
		interaction.current = null;
		event.currentTarget.releasePointerCapture?.(event.pointerId);
	}

	return { begin, move, stop };
}

/** Screen paperwork uses the same geometry as print with white text on a dark ground. */
function PrintTitleBlock({ page, documentInfo }: Pick<PrintFrameProps, "page" | "documentInfo">) {
	const info: CadPrintDocumentInfo = {
		showName: "", lightingDesigner: "", showVersion: "", venue: "", contactEmail: "",
		contactPhone: "", project: "", showDate: "", lastSavedAt: 0, fixtureCount: 0,
		universeCount: 0, ...documentInfo,
	};
	const logo = parseCompanyLogo(info.companyLogo);
	return (
		<svg className="cad-print-title-block" viewBox={`0 0 ${TITLE_W} ${TITLE_H}`} role="img" aria-label={`${page.name} information`}>
			<rect x="0" y="0" width={TITLE_W} height={TITLE_H} fill="#0b0d10" stroke="#f5f5f5" strokeWidth="0.9" />
			<line x1={PRINT_TITLE_DIVIDER_X} x2={PRINT_TITLE_DIVIDER_X} y1="0" y2={TITLE_H} stroke="#f5f5f5" strokeWidth="0.9" />
			<line x1="0" x2={PRINT_TITLE_DIVIDER_X} y1={TITLE_H - printTitleSeparatorY(page, info)} y2={TITLE_H - printTitleSeparatorY(page, info)} stroke="#f5f5f5" strokeWidth="0.9" />
			<image href={logo ? companyLogoUrl(logo) : architectIconUrl}
				aria-label={logo ? "Company logo" : "ToskLight vector logo with blue accent"}
				x={PRINT_TITLE_MARK.x} y={TITLE_H - PRINT_TITLE_MARK.y - PRINT_TITLE_MARK.size}
				width={PRINT_TITLE_MARK.size} height={PRINT_TITLE_MARK.size} preserveAspectRatio="xMidYMid meet" />
			{printTitleEntries(page, info).map((entry, index) => {
				const fitted = fitTitleText(entry.value, entry.size, entry.width, entry.bold);
				return <text key={`${index}:${entry.value}`} x={entry.x} y={TITLE_H - entry.y}
					fontSize={fitted.size} fontWeight={entry.bold ? "bold" : "normal"}
					fontFamily="Helvetica, Arial, sans-serif" fill={entry.muted ? "#a0a0a0" : "#f5f5f5"}>{fitted.label}</text>;
			})}
		</svg>
	);
}

/**
 * The drawings this page prints, drawn on the sheet itself.
 *
 * The tile underneath already draws every visible drawing of the view; this layer is what the page
 * will actually print, so a page that switches one off looks on screen like the PDF it exports.
 */
function PrintPageUnderlays({
	page,
	underlays,
	camera,
	height,
	millimetrePixels,
}: {
	page: CadPrintPage;
	underlays: readonly CadUnderlay[];
	camera: TileCamera;
	height: number;
	millimetrePixels: number;
}) {
	const width = page.widthMillimetres;
	const border = PRINT_BORDER_MM * millimetrePixels;
	const runs = underlaysForPage(underlays, page).flatMap((underlay) =>
		placedPolylines(underlay, page.rotationQuarterTurns),
	);
	if (!runs.length) return null;
	const x = (value: number) =>
		(value - page.centreMillimetres[0] + width / 2) * camera.zoom - border;
	const y = (value: number) =>
		(page.centreMillimetres[1] + height / 2 - value) * camera.zoom - border;
	return (
		<svg
			className="cad-print-underlays"
			aria-hidden="true"
			viewBox={`0 0 ${width * camera.zoom} ${height * camera.zoom}`}
			width={width * camera.zoom}
			height={height * camera.zoom}
		>
			{runs.map((run, index) => (
				<polyline
					// Runs have no identity of their own; their order within a page is stable.
					// biome-ignore lint/suspicious/noArrayIndexKey: positional by nature
					key={index}
					points={run.map((point) => `${x(point[0])},${y(point[1])}`).join(" ")}
				/>
			))}
		</svg>
	);
}

/** One print page drawn over the tile it belongs to, as the sheet it will become. */
export function PrintFrame({
	page,
	entities,
	underlays,
	camera,
	viewportRotation = 0,
	selected,
	onSelect,
	onChange,
	onCamera,
	documentInfo,
}: PrintFrameProps) {
	const { begin, move, stop } = usePrintFrameInteraction({
		page,
		camera,
		viewportRotation,
		onChange,
		onCamera,
	});
	const frame = pageFrameGeometry(page, viewportRotation);
	const height = printPageHeight(page);
	const paper = printPaperSize(page);
	const millimetrePixels = (page.widthMillimetres * camera.zoom) / paper.width;
	const gridSize = printGridMillimetres(page) * camera.zoom;
	const grab =
		(gesture: FrameGesture) => (event: React.PointerEvent<HTMLElement>) => {
			if (event.button !== 0 && event.button !== 1) return;
			event.preventDefault();
			event.stopPropagation();
			if (event.button === 0) onSelect();
			begin(gesture, event);
		};
	return (
		<div
			className={`cad-print-frame ${selected ? "is-selected" : ""}`}
			style={{
				left: `calc(50% + ${(frame.centreMillimetres[0] - page.widthMillimetres / 2 + camera.pan[0]) * camera.zoom}px)`,
				top: `calc(50% - ${(frame.centreMillimetres[1] + height / 2 + camera.pan[1]) * camera.zoom}px)`,
				transform: `rotate(${frame.rotationDegrees}deg)`,
				transformOrigin: "center",
				zIndex: selected ? 2 : 1,
				width: `${page.widthMillimetres * camera.zoom}px`,
				height: `${height * camera.zoom}px`,
			}}
			onPointerDown={grab("move")}
			onPointerMove={move}
			onPointerUp={stop}
			onPointerCancel={stop}
		>
			<PrintPageSettings {...{ page, entities, underlays, onChange }} />
			<div
				className="cad-print-sheet"
				style={
					{
						"--cad-print-grid": `${gridSize}px`,
						"--cad-print-border": `${PRINT_BORDER_MM * millimetrePixels}px`,
						"--cad-print-title-width": `${PRINT_TITLE_WIDTH_MM * millimetrePixels}px`,
						"--cad-print-title-height": `${PRINT_TITLE_HEIGHT_MM * millimetrePixels}px`,
					} as React.CSSProperties
				}
				aria-hidden="true"
			>
				<div className="cad-print-page-name">{page.name}</div>
				<PrintPageUnderlays
					{...{ page, underlays, camera, height, millimetrePixels }}
				/>
				<PrintPageLabels
					{...{ page, entities, camera, height, millimetrePixels }}
				/>
				<PrintTitleBlock {...{ page, documentInfo }} />
			</div>
			<button
				type="button"
				className="cad-print-scale"
				aria-label={page.fixedScaleDenominator === undefined ? `Scale ${page.name}` : `Scale locked at 1:${page.fixedScaleDenominator}`}
				disabled={page.fixedScaleDenominator !== undefined}
				title={page.fixedScaleDenominator === undefined ? "Resize print frame" : "Disable Fixed scale in Plans to resize this page"}
				onPointerDown={grab("scale")}
				onPointerMove={move}
				onPointerUp={stop}
				onPointerCancel={stop}
			/>
		</div>
	);
}
