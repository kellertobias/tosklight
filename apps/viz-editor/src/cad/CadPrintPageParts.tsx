import { FIXTURE_LABEL_LINE_HEIGHT, FIXTURE_LABEL_PADDING, FIXTURE_LABEL_SIZE, printFixtureLabelLayout } from "./printFixtureLabels";
import { LABEL_CAPACITY_MESSAGE } from "./labelPlacement";
import { Button, SwitchField } from "@tosklight/ui";
import { useMemo, useState } from "react";
import { CadDepthMenu } from "./CadDepthMenu";
import { PRINT_BORDER_MM } from "./print";
import {
	type CadEntity,
	type CadPrintPage,
	type TileCamera,
} from "./types";
import {
	pageShowsUnderlay,
	underlaysForView,
	withUnderlayShown,
} from "./underlayGeometry";
import type { CadUnderlay } from "./underlays";

/// The cogwheel in the corner of a page, and the settings it opens for that page alone.
export function PrintPageSettings({
	page,
	entities,
	underlays = [],
	onChange,
}: {
	page: CadPrintPage;
	entities: readonly CadEntity[];
	underlays?: readonly CadUnderlay[];
	onChange(change: Partial<CadPrintPage>): void;
}) {
	// Open only while an operator is working on this page. The cut applies while the menu is open
	// so the page shows what it is about to print; the export reads the same numbers either way.
	const [rangeOpen, setRangeOpen] = useState(false);
	return (
		<>
			<Button
				className="cad-print-page-settings-toggle"
				aria-label={`${page.name} page settings`}
				title="Page settings"
				active={rangeOpen}
				onPointerDown={(event) => event.stopPropagation()}
				onClick={() => setRangeOpen((current) => !current)}
			>
				<svg viewBox="0 0 16 16" aria-hidden="true">
					<circle cx="8" cy="8" r="2.4" />
					<path d="M8 1.5v2M8 12.5v2M1.5 8h2M12.5 8h2M3.4 3.4l1.4 1.4M11.2 11.2l1.4 1.4M12.6 3.4l-1.4 1.4M4.8 11.2l-1.4 1.4" />
				</svg>
			</Button>
			{rangeOpen ? (
				<div
					className="cad-print-page-menu"
					onPointerDown={(event) => event.stopPropagation()}
				>
					<CadDepthMenu
						view={page.view}
						entities={entities}
						cutPlanes={page.cutPlanes}
						onChange={(cutPlanes) => onChange({ cutPlanes })}
						onClose={() => setRangeOpen(false)}
					>
						<SwitchField
							label="Fixture IDs"
							offLabel={null}
							onLabel={null}
							checked={page.showFixtureIds}
							onChange={(event) =>
								onChange({ showFixtureIds: event.currentTarget.checked })
							}
						/>
						<SwitchField
							label="DMX patch"
							offLabel={null}
							onLabel={null}
							checked={page.showDmxAddresses}
							onChange={(event) =>
								onChange({ showDmxAddresses: event.currentTarget.checked })
							}
						/>
						<SwitchField
							label="Mounting hardware"
							offLabel={null}
							onLabel={null}
							checked={page.showMountingHardware !== false}
							onChange={(event) =>
								onChange({ showMountingHardware: event.currentTarget.checked })
							}
						/>
						{underlaysForView(underlays, page.view).map((underlay) => (
							<SwitchField
								key={underlay.id}
								label={underlay.name}
								offLabel={null}
								onLabel={null}
								checked={pageShowsUnderlay(page, underlay.id)}
								onChange={(event) =>
									onChange({
										hiddenUnderlayIds: withUnderlayShown(
											page,
											underlay.id,
											event.currentTarget.checked,
										),
									})
								}
							/>
						))}
					</CadDepthMenu>
				</div>
			) : null}
		</>
	);
}

/// The fixture identities a page prints over its drawing, for the ones that land on the sheet.
export function PrintPageLabels({
	page,
	entities,
	camera,
	height,
	millimetrePixels,
}: {
	page: CadPrintPage;
	entities: readonly CadEntity[];
	camera: TileCamera;
	height: number;
	millimetrePixels: number;
}) {
	const layout = useMemo(() => printFixtureLabelLayout(page, entities), [page, entities]);
	if (!page.showFixtureIds && !page.showDmxAddresses) return null;
	return <>
		<svg className="cad-print-labels" aria-label="Fixture labels"
			viewBox={`0 0 ${layout.widthPoints} ${layout.heightPoints}`}
			style={{ left: -PRINT_BORDER_MM * millimetrePixels, top: -PRINT_BORDER_MM * millimetrePixels, width: page.widthMillimetres * camera.zoom, height: height * camera.zoom }}>
			<g stroke="#00b6eb" strokeWidth={0.3}>
				{layout.labels.map((label) => <line key={label.key} x1={label.leader[0][0]} y1={label.leader[0][1]} x2={label.leader[1][0]} y2={label.leader[1][1]} />)}
			</g>
			{layout.labels.map((label) => <g key={label.key} data-fixture-instance={label.key}>
				<rect x={label.box.x} y={label.box.y} width={label.box.width} height={label.box.height} fill="#0b0d10" />
				{label.lines.map((line, index) => <text key={index}
					x={label.box.x + FIXTURE_LABEL_PADDING} y={label.box.y + FIXTURE_LABEL_PADDING + FIXTURE_LABEL_SIZE + index * FIXTURE_LABEL_LINE_HEIGHT}
					fontFamily="Helvetica, Arial, sans-serif" fontSize={FIXTURE_LABEL_SIZE} fontWeight="bold" fill="#00b6eb">{line}</text>)}
			</g>)}
		</svg>
		{layout.unplaced.length ? <div role="status" className="cad-label-capacity-warning">
			{layout.unplaced.length} labels do not fit. {LABEL_CAPACITY_MESSAGE}
		</div> : null}
	</>;
}
