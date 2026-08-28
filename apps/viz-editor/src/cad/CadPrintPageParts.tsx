import { Button, SwitchField } from "@tosklight/ui";
import { useState } from "react";
import { CadDepthMenu } from "./CadDepthMenu";
import { PRINT_BORDER_MM } from "./print";
import {
	type CadEntity,
	type CadPrintPage,
	projectPoint,
	type TileCamera,
} from "./types";

/// The cogwheel in the corner of a page, and the settings it opens for that page alone.
export function PrintPageSettings({
	page,
	entities,
	onChange,
}: {
	page: CadPrintPage;
	entities: readonly CadEntity[];
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
	return (
		<>
				{page.showFixtureIds || page.showDmxAddresses ? (
			<div className="cad-print-labels">
				{entities
					.filter((entity) => entity.kind !== "venue")
					.map((entity) => {
						const point = projectPoint(
							entity.positionMillimetres,
							page.view,
							page.rotationQuarterTurns,
						);
						const x =
							(point[0] -
								page.centreMillimetres[0] +
								page.widthMillimetres / 2) *
								camera.zoom -
							PRINT_BORDER_MM * millimetrePixels;
						const y =
							(page.centreMillimetres[1] + height / 2 - point[1]) *
								camera.zoom -
							PRINT_BORDER_MM * millimetrePixels;
						if (
							x < 0 ||
							y < 0 ||
							x > page.widthMillimetres * camera.zoom ||
							y > height * camera.zoom
						)
							return null;
						return (
							<span key={entity.id} style={{ left: x, top: y }}>
								{page.showFixtureIds
									? `ID ${entity.fixtureDisplayId}`
									: null}
								{page.showFixtureIds && page.showDmxAddresses
									? " · "
									: null}
								{page.showDmxAddresses && entity.dmxAddress !== "—"
									? `DMX ${entity.dmxAddress}`
									: null}
							</span>
						);
					})}
			</div>
		) : null}
		</>
	);
}
