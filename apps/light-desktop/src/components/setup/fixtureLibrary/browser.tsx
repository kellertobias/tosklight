import type { FixtureDefinition } from "../../../api/types";
import { Button, Select } from "../../common";
import { WindowScrollArea } from "../../window-kit";
import { fixtureDefinitionKey } from "../fixtureProfileModel";
import type { FixtureLibraryFamily } from "./model";

interface FixtureLibraryBrowserProps {
	libraryFamilies: FixtureLibraryFamily[];
	manufacturer: string;
	manufacturers: string[];
	selectedFamily: FixtureLibraryFamily | null;
	selectedMode: FixtureDefinition | null;
	onEdit: (mode: FixtureDefinition) => void;
	onExport: () => void;
	onRevisionHistory: () => void;
	setManufacturer: (manufacturer: string) => void;
	setSelectedFamilyKey: (key: string) => void;
	setSelectedModeKey: (key: string) => void;
}

export function FixtureLibraryBrowser({
	libraryFamilies,
	manufacturer,
	manufacturers,
	selectedFamily,
	selectedMode,
	onEdit,
	onExport,
	onRevisionHistory,
	setManufacturer,
	setSelectedFamilyKey,
	setSelectedModeKey,
}: FixtureLibraryBrowserProps) {
	return (
		<div className="fixture-library-columns">
			<section>
				<h3>Manufacturer</h3>
				<WindowScrollArea className="fixture-library-column-scroll">
					<Button
						className={!manufacturer ? "active" : ""}
						onClick={() => setManufacturer("")}
					>
						<span>All manufacturers</span>
					</Button>
					{manufacturers.map((name) => (
						<Button
							className={manufacturer === name ? "active" : ""}
							key={name}
							onClick={() => setManufacturer(name)}
						>
							<span>{name}</span>
						</Button>
					))}
				</WindowScrollArea>
			</section>
			<section>
				<h3>Fixture</h3>
				<WindowScrollArea className="fixture-library-column-scroll">
					{libraryFamilies.map((family) => (
						<Button
							className={selectedFamily?.key === family.key ? "active" : ""}
							key={family.key}
							onClick={() => {
								setSelectedFamilyKey(family.key);
								setSelectedModeKey(fixtureDefinitionKey(family.modes[0]));
							}}
						>
							<span>{family.name}</span>
							<small>
								{family.deviceType} · {family.modes.length} modes
							</small>
						</Button>
					))}
				</WindowScrollArea>
			</section>
			<section className="fixture-library-detail">
				<WindowScrollArea className="fixture-library-column-scroll">
					{selectedFamily && selectedMode ? (
						<FixtureLibraryDetail
							family={selectedFamily}
							mode={selectedMode}
							onEdit={onEdit}
							onExport={onExport}
							onRevisionHistory={onRevisionHistory}
							setSelectedModeKey={setSelectedModeKey}
						/>
					) : (
						<p>No fixture matches this search.</p>
					)}
				</WindowScrollArea>
			</section>
		</div>
	);
}

function FixtureLibraryDetail({
	family,
	mode,
	onEdit,
	onExport,
	onRevisionHistory,
	setSelectedModeKey,
}: {
	family: FixtureLibraryFamily;
	mode: FixtureDefinition;
	onEdit: (mode: FixtureDefinition) => void;
	onExport: () => void;
	onRevisionHistory: () => void;
	setSelectedModeKey: (key: string) => void;
}) {
	return (
		<>
			<h3>
				{family.manufacturer} {family.name}
			</h3>
			<label htmlFor="fixture-library-mode">
				Mode
				<Select
					id="fixture-library-mode"
					value={fixtureDefinitionKey(mode)}
					onChange={(event) => setSelectedModeKey(event.target.value)}
				>
					{family.modes.map((candidate) => (
						<option
							value={fixtureDefinitionKey(candidate)}
							key={fixtureDefinitionKey(candidate)}
						>
							{candidate.mode} · {candidate.footprint}ch
						</option>
					))}
				</Select>
			</label>
			<dl>
				<dt>Type</dt>
				<dd>{mode.device_type}</dd>
				<dt>DMX footprint</dt>
				<dd>{mode.footprint} channels</dd>
				<dt>Heads</dt>
				<dd>{mode.heads.length}</dd>
				<dt>Revision</dt>
				<dd>{mode.revision}</dd>
				<dt>Physical</dt>
				<dd>
					{mode.physical.width_millimetres ?? "?"} ×{" "}
					{mode.physical.height_millimetres ?? "?"} ×{" "}
					{mode.physical.depth_millimetres ?? "?"} mm
				</dd>
			</dl>
			<Button onClick={() => onEdit(mode)}>Edit fixture</Button>
			<Button onClick={onRevisionHistory}>Revision history</Button>
			<Button onClick={onExport}>Export fixture</Button>
		</>
	);
}
