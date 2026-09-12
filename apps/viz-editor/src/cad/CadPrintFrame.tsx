import { Fragment, useRef } from "react";
import architectIconUrl from "../../../../assets/branding/tosklight-icon-print.svg";
import { PrintPageLabels, PrintPageSettings } from "./CadPrintPageParts";
import {
	type CadPrintDocumentInfo,
	PRINT_BORDER_MM,
	PRINT_TITLE_HEIGHT_MM,
	PRINT_TITLE_WIDTH_MM,
	printGridMillimetres,
	printScaleDenominator,
} from "./print";
import type { CadEntity, CadPrintPage, TileCamera } from "./types";
import { printPageHeight, printPaperSize } from "./types";
import { placedPolylines, underlaysForPage } from "./underlayGeometry";
import type { CadUnderlay } from "./underlays";

interface PrintFrameProps {
	page: CadPrintPage;
	entities: readonly CadEntity[];
	underlays: readonly CadUnderlay[];
	camera: TileCamera;
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
	onChange,
	onCamera,
}: Pick<PrintFrameProps, "page" | "camera" | "onChange" | "onCamera">) {
	const interaction = useRef<{
		type: FrameGesture;
		start: [number, number];
		centre: [number, number];
		width: number;
		camera: TileCamera;
	} | null>(null);

	function begin(gesture: FrameGesture, event: React.PointerEvent<HTMLElement>) {
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
					active.centre[0] + dx / camera.zoom,
					active.centre[1] - dy / camera.zoom,
				],
			});
		} else {
			onChange({
				widthMillimetres: Math.max(
					500,
					active.width +
						Math.max(
							(dx * 2) / camera.zoom,
							(dy * 2 * 297) / (camera.zoom * 210),
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

/** The paperwork corner of a sheet: who drew it, for which show, at what scale. */
function PrintTitleBlock({
	page,
	documentInfo,
}: Pick<PrintFrameProps, "page" | "documentInfo">) {
	const rows: [string, string][] = [
		...(documentInfo?.showName ? [["Show", documentInfo.showName] as [string, string]] : []),
		...(documentInfo?.lightingDesigner
			? [["Designer", documentInfo.lightingDesigner] as [string, string]]
			: []),
		...(documentInfo?.venue ? [["Venue", documentInfo.venue] as [string, string]] : []),
		...(documentInfo?.showDate ? [["Show date", documentInfo.showDate] as [string, string]] : []),
		...(documentInfo?.contactEmail
			? [["Email", documentInfo.contactEmail] as [string, string]]
			: []),
		...(documentInfo?.contactPhone
			? [["Phone", documentInfo.contactPhone] as [string, string]]
			: []),
		...(documentInfo?.showVersion
			? [["Version", documentInfo.showVersion] as [string, string]]
			: []),
		["Scale", `1:${printScaleDenominator(page)}`],
		[
			"Rig",
			`${documentInfo?.fixtureCount ?? 0} fixtures · ${documentInfo?.universeCount ?? 0} universes`,
		],
	];
	return (
		<div className="cad-print-title-block">
			<img src={architectIconUrl} alt="ToskLight application icon" />
			<div className="cad-print-brand">
				<strong>ToskLight Architect</strong>
				{documentInfo?.project ? <span>{documentInfo.project}</span> : null}
			</div>
			<div className="cad-print-meta">
				{rows.map(([label, value]) => (
					<Fragment key={label}>
						<span>{label}</span>
						<strong>{value}</strong>
					</Fragment>
				))}
			</div>
		</div>
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
	selected,
	onSelect,
	onChange,
	onCamera,
	documentInfo,
}: PrintFrameProps) {
	const { begin, move, stop } = usePrintFrameInteraction({
		page,
		camera,
		onChange,
		onCamera,
	});
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
				left: `calc(50% + ${(page.centreMillimetres[0] - page.widthMillimetres / 2 + camera.pan[0]) * camera.zoom}px)`,
				top: `calc(50% - ${(page.centreMillimetres[1] + height / 2 + camera.pan[1]) * camera.zoom}px)`,
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
				aria-label={`Scale ${page.name}`}
				onPointerDown={grab("scale")}
				onPointerMove={move}
				onPointerUp={stop}
				onPointerCancel={stop}
			/>
		</div>
	);
}
