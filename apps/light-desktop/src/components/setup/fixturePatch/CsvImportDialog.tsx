import {
	Button,
	Input,
	ModalRegistration,
	ModalTitleBar,
	MultiValueToggleField,
	SelectField,
	TextInput,
} from "@tosklight/ui";
import { useMemo, useState } from "react";
import type { FixtureDefinition } from "../../../api/types";
import { normalizeFixtureSearch } from "../fixtureLibrary/model";
import { fixtureDefinitionKey } from "../fixtureProfileModel";
import { compareFixtureManufacturers, groupFixtureFamilies } from "../patchUtils";
import { type PatchController, usePatchController } from "./controller";
import {
	CSV_IMPORT_FIELDS,
	type CsvConflictPolicy,
	type CsvImportField,
	type CsvImportRowPlan,
	type CsvPositionUnit,
	type CsvSourceType,
	type CsvTypeMapping,
	describeSourceType,
	type ParsedCsv,
} from "./csvImport";
import { runCsvImport, useCsvImportPlan } from "./csvImportPlan";
import { type CsvFirstRow, useCsvImportSource } from "./csvImportSource";
import { FixtureDetails, FixtureModeSelect } from "./fixtureDisplay";

type Step = "columns" | "types" | "review";

const PREVIEW_ROWS = 8;

const STEP_TITLES: Record<Step, string> = {
	columns: "Import CSV · Columns",
	types: "Import CSV · Fixture types",
	review: "Import CSV · Review",
};

export function CsvImportDialog() {
	const controller = usePatchController();
	if (!controller.ui.csvImportOpen) return null;
	return <CsvImport controller={controller} />;
}

function CsvImport({ controller }: { controller: PatchController }) {
	const [conflictPolicy, setConflictPolicy] =
		useState<CsvConflictPolicy>("unpatch");
	const [step, setStep] = useState<Step>("columns");
	const [mappings, setMappings] = useState<Record<string, CsvTypeMapping>>({});
	const [activeTypeKey, setActiveTypeKey] = useState<string | null>(null);
	const [busy, setBusy] = useState(false);
	const [importError, setImportError] = useState("");
	const [closeConfirm, setCloseConfirm] = useState(false);
	const source = useCsvImportSource(() => {
		setMappings({});
		setImportError("");
		setStep("columns");
	});
	const { sourceTypes, unresolved, plans, importable, layerId, layerName } =
		useCsvImportPlan({ controller, source, mappings, conflictPolicy });

	const close = () => controller.ui.setCsvImportOpen(false);
	const requestClose = () => (source.parsed ? setCloseConfirm(true) : close());

	const chooseMapping = (key: string, mapping: CsvTypeMapping) => {
		const next = { ...mappings, [key]: mapping };
		setMappings(next);
		const following = sourceTypes.find(
			(item) => item.key !== key && !(next[item.key] ?? item.exactMatch),
		);
		if (following) setActiveTypeKey(following.key);
	};

	const runImport = async () => {
		setBusy(true);
		setImportError("");
		const error = await runCsvImport(controller, plans, layerId, source.fileName);
		setBusy(false);
		if (error) setImportError(error);
	};

	const actions = csvImportTitleActions({
		step,
		hasRows: source.dataRows.length > 0,
		unresolvedCount: unresolved.length,
		importableCount: importable.length,
		busy,
		onCancel: requestClose,
		onContinue: () => {
			setActiveTypeKey(unresolved[0]?.key ?? null);
			setStep(unresolved.length ? "types" : "review");
		},
		onBackToColumns: () => setStep("columns"),
		onReview: () => setStep("review"),
		onBackToTypes: () => {
			setActiveTypeKey(
				activeTypeKey ?? unresolved[0]?.key ?? sourceTypes[0]?.key ?? null,
			);
			setStep("types");
		},
		onImport: () => void runImport(),
	});

	return (
		<ModalRegistration onClose={requestClose}>
			<div className="stacked-modal-layer">
				<section
					className="nested-modal csv-import-modal"
					role="dialog"
					aria-modal="true"
					aria-labelledby="csv-import-title"
				>
					<ModalTitleBar
						title={STEP_TITLES[step]}
						titleId="csv-import-title"
						details={source.fileName || undefined}
						groups={[{ id: "csv-import-actions", actions }]}
						closeLabel="Close Import CSV"
						onClose={requestClose}
					/>
					{step === "columns" && (
						<ColumnsStep
							fileError={source.fileError}
							parsed={source.parsed}
							headers={source.headers}
							dataRows={source.dataRows}
							assignments={source.assignments}
							firstRow={source.firstRow}
							positionUnit={source.positionUnit}
							onFile={(file) => void source.selectFile(file)}
							onFirstRow={source.setFirstRow}
							onPositionUnit={source.setPositionUnit}
							onAssign={source.assign}
						/>
					)}
					{step === "types" && (
						<TypesStep
							sourceTypes={sourceTypes}
							mappings={mappings}
							activeKey={activeTypeKey ?? sourceTypes[0]?.key ?? null}
							definitions={controller.data.availableDefinitions}
							unresolvedCount={unresolved.length}
							onActivate={setActiveTypeKey}
							onChoose={chooseMapping}
						/>
					)}
					{step === "review" && (
						<ReviewStep
							plans={plans}
							layerName={layerName}
							conflictPolicy={conflictPolicy}
							importError={importError}
							onConflictPolicy={setConflictPolicy}
						/>
					)}
				</section>
				{closeConfirm && (
					<CsvImportCloseConfirm
						fileName={source.fileName}
						onClose={close}
						onStay={() => setCloseConfirm(false)}
					/>
				)}
			</div>
		</ModalRegistration>
	);
}

function csvImportTitleActions(options: {
	step: Step;
	hasRows: boolean;
	unresolvedCount: number;
	importableCount: number;
	busy: boolean;
	onCancel: () => void;
	onContinue: () => void;
	onBackToColumns: () => void;
	onReview: () => void;
	onBackToTypes: () => void;
	onImport: () => void;
}) {
	if (options.step === "columns")
		return [
			{ id: "cancel", label: "Cancel", onPress: options.onCancel },
			{
				id: "next",
				label: "Next: fixture types",
				variant: "primary" as const,
				disabled: !options.hasRows,
				onPress: options.onContinue,
			},
		];
	if (options.step === "types")
		return [
			{ id: "back", label: "Back", onPress: options.onBackToColumns },
			{
				id: "next",
				label: "Next: review",
				variant: "primary" as const,
				disabled: options.unresolvedCount > 0,
				onPress: options.onReview,
			},
		];
	const count = options.importableCount;
	return [
		{ id: "back", label: "Back", onPress: options.onBackToTypes },
		{
			id: "import",
			label: options.busy
				? "Importing…"
				: `Import ${count} fixture${count === 1 ? "" : "s"}`,
			variant: "primary" as const,
			disabled: options.busy || count === 0,
			onPress: options.onImport,
		},
	];
}

function CsvImportCloseConfirm({
	fileName,
	onClose,
	onStay,
}: {
	fileName: string;
	onClose: () => void;
	onStay: () => void;
}) {
	return (
		<ModalRegistration onClose={onStay}>
			<div className="stacked-modal-layer">
				<section
					className="nested-modal patch-small-modal"
					role="dialog"
					aria-modal="true"
					aria-labelledby="close-csv-import-title"
				>
					<ModalTitleBar
						title="Close Import CSV?"
						titleId="close-csv-import-title"
						onClose={onStay}
					/>
					<p>No fixtures from {fileName} have been imported yet.</p>
					<footer>
						<Button className="danger" onClick={onClose}>
							Yes, close
						</Button>
						<Button onClick={onStay}>Stay in Import CSV</Button>
					</footer>
				</section>
			</div>
		</ModalRegistration>
	);
}

function ColumnsStep({
	fileError,
	parsed,
	headers,
	dataRows,
	assignments,
	firstRow,
	positionUnit,
	onFile,
	onFirstRow,
	onPositionUnit,
	onAssign,
}: {
	fileError: string;
	parsed: ParsedCsv | null;
	headers: readonly string[];
	dataRows: readonly string[][];
	assignments: ReadonlyArray<CsvImportField | null>;
	firstRow: CsvFirstRow;
	positionUnit: CsvPositionUnit;
	onFile: (file?: File) => void;
	onFirstRow: (value: CsvFirstRow) => void;
	onPositionUnit: (value: CsvPositionUnit) => void;
	onAssign: (column: number, field: CsvImportField | null) => void;
}) {
	const assignedColumn = (field: CsvImportField) => {
		const index = assignments.indexOf(field);
		return index < 0 ? null : headers[index];
	};
	return (
		<div className="csv-import-body csv-import-columns">
			<div className="csv-import-options">
				<label htmlFor="patch-csv-file">
					CSV file
					<Input
						id="patch-csv-file"
						type="file"
						accept=".csv,.txt,text/csv"
						onChange={(event) => onFile(event.target.files?.[0])}
					/>
				</label>
				{parsed && (
					<>
						<MultiValueToggleField
							label="First row"
							value={firstRow}
							options={[
								{ value: "header", label: "Column names" },
								{ value: "data", label: "Fixture data" },
							]}
							onChange={onFirstRow}
						/>
						<MultiValueToggleField
							label="X / Y / Z unit"
							value={positionUnit}
							options={[
								{ value: "m", label: "Metres" },
								{ value: "mm", label: "Millimetres" },
							]}
							onChange={onPositionUnit}
						/>
					</>
				)}
			</div>
			{fileError && (
				<p className="patch-status" role="alert">
					{fileError}
				</p>
			)}
			{!parsed && !fileError && (
				<p className="patch-secondary">
					Choose a comma-, semicolon-, or tab-separated file. Each row becomes
					one fixture.
				</p>
			)}
			{parsed && (
				<>
					<ul className="csv-import-field-summary" aria-label="Column assignments">
						{CSV_IMPORT_FIELDS.map(({ field, label }) => {
							const column = assignedColumn(field);
							return (
								<li key={field} className={column ? "assigned" : ""}>
									<b>{label}</b>
									<span>{column ?? "Not assigned"}</span>
								</li>
							);
						})}
					</ul>
					{!assignments.includes("fixture_type") && (
						<p className="patch-secondary">
							No column is assigned to Fixture Type. You will choose one library
							fixture for every row in the next step.
						</p>
					)}
					<div className="csv-import-table-scroll">
						<table className="csv-import-table">
							<thead>
								<tr>
									{headers.map((header, index) => (
										<th key={`assign-${index}`}>
											<SelectField<CsvImportField | "">
												ariaLabel={`Assign column ${header}`}
												value={assignments[index] ?? ""}
												options={[
													{ value: "", label: "Ignore" },
													...CSV_IMPORT_FIELDS.map(({ field, label }) => ({
														value: field,
														label,
													})),
												]}
												onChange={(value) => onAssign(index, value || null)}
											/>
										</th>
									))}
								</tr>
								<tr>
									{headers.map((header, index) => (
										<th key={`header-${index}`}>{header}</th>
									))}
								</tr>
							</thead>
							<tbody>
								{dataRows.slice(0, PREVIEW_ROWS).map((row, rowIndex) => (
									<tr key={rowIndex}>
										{headers.map((_, index) => (
											<td key={index}>{row[index] ?? ""}</td>
										))}
									</tr>
								))}
							</tbody>
						</table>
					</div>
					<p className="patch-secondary">
						{dataRows.length} fixture row{dataRows.length === 1 ? "" : "s"}
						{dataRows.length > PREVIEW_ROWS
							? ` · showing the first ${PREVIEW_ROWS}`
							: ""}
					</p>
				</>
			)}
		</div>
	);
}

function TypesStep({
	sourceTypes,
	mappings,
	activeKey,
	definitions,
	unresolvedCount,
	onActivate,
	onChoose,
}: {
	sourceTypes: readonly CsvSourceType[];
	mappings: Readonly<Record<string, CsvTypeMapping>>;
	activeKey: string | null;
	definitions: readonly FixtureDefinition[];
	unresolvedCount: number;
	onActivate: (key: string) => void;
	onChoose: (key: string, mapping: CsvTypeMapping) => void;
}) {
	const active = sourceTypes.find((source) => source.key === activeKey) ?? null;
	return (
		<div className="csv-import-body csv-import-types">
			<aside aria-label="Fixture types in the file">
				<p className="patch-secondary">
					{unresolvedCount
						? `${unresolvedCount} of ${sourceTypes.length} fixture types need a library fixture.`
						: "Every fixture type has a library fixture."}
				</p>
				{sourceTypes.map((source) => {
					const mapping = mappings[source.key];
					const state =
						mapping === "skip"
							? "Skipped"
							: mapping
								? `Selected: ${definitionLabel(mapping)}`
								: source.exactMatch
									? `Exact match: ${definitionLabel(source.exactMatch)}`
									: "Needs a library fixture";
					return (
						<Button
							key={source.key}
							className={`${source.key === activeKey ? "active" : ""} ${!mapping && !source.exactMatch ? "csv-import-unresolved" : ""}`.trim()}
							onClick={() => onActivate(source.key)}
						>
							<span>{describeSourceType(source)}</span>
							<small>
								{source.rowCount} row{source.rowCount === 1 ? "" : "s"} · {state}
							</small>
						</Button>
					);
				})}
			</aside>
			{active && (
				<LibraryFixturePicker
					key={active.key}
					source={active}
					current={mappings[active.key] ?? active.exactMatch}
					definitions={definitions}
					onUse={(definition) => onChoose(active.key, definition)}
					onSkip={() => onChoose(active.key, "skip")}
				/>
			)}
		</div>
	);
}

function definitionLabel(definition: FixtureDefinition) {
	return `${definition.manufacturer} ${definition.name || definition.model} · ${definition.mode}`;
}

function familyKeyOf(definition: FixtureDefinition) {
	return `${definition.manufacturer}\0${definition.model || definition.name}`;
}

function normalizeName(value: string) {
	return value.trim().replace(/\s+/g, " ").toLocaleLowerCase();
}

/** Preselects the most plausible library fixture; the operator still confirms it explicitly. */
function suggestedDefinition(
	families: ReturnType<typeof groupFixtureFamilies>,
	source: CsvSourceType,
): FixtureDefinition | null {
	const type = normalizeName(source.fixtureType);
	if (!type) return null;
	const manufacturer = normalizeName(source.manufacturer);
	const names = (family: (typeof families)[number]) =>
		[family.name, ...family.modes.map((mode) => mode.model)].map(normalizeName);
	const ranked = families
		.map((family) => {
			const familyNames = names(family);
			const nameScore = familyNames.includes(type)
				? 2
				: familyNames.some(
							(name) => name && (name.includes(type) || type.includes(name)),
						)
					? 1
					: 0;
			const manufacturerScore =
				manufacturer && normalizeName(family.manufacturer) === manufacturer
					? 1
					: 0;
			return { family, score: nameScore * 2 + manufacturerScore, nameScore };
		})
		.filter((item) => item.nameScore > 0)
		.sort((a, b) => b.score - a.score);
	const family = ranked[0]?.family;
	if (!family) return null;
	const mode = normalizeName(source.mode);
	return (
		family.modes.find((candidate) => normalizeName(candidate.mode) === mode) ??
		family.modes[0] ??
		null
	);
}

function LibraryFixturePicker({
	source,
	current,
	definitions,
	onUse,
	onSkip,
}: {
	source: CsvSourceType;
	current: CsvTypeMapping | null;
	definitions: readonly FixtureDefinition[];
	onUse: (definition: FixtureDefinition) => void;
	onSkip: () => void;
}) {
	const families = useMemo(
		() => groupFixtureFamilies([...definitions]),
		[definitions],
	);
	const [initial] = useState(() =>
		current && current !== "skip"
			? current
			: suggestedDefinition(families, source),
	);
	const [query, setQuery] = useState("");
	const [manufacturer, setManufacturer] = useState("");
	const [familyKey, setFamilyKey] = useState(
		initial ? familyKeyOf(initial) : "",
	);
	const [definitionKey, setDefinitionKey] = useState(
		initial ? fixtureDefinitionKey(initial) : "",
	);
	const manufacturers = useMemo(
		() =>
			[...new Set(families.map((family) => family.manufacturer))].sort(
				compareFixtureManufacturers,
			),
		[families],
	);
	const needle = normalizeFixtureSearch(query);
	const filtered = families.filter(
		(family) =>
			(!manufacturer || family.manufacturer === manufacturer) &&
			(!needle ||
				normalizeFixtureSearch(
					`${family.manufacturer} ${family.name} ${family.deviceType} ${family.modes.map((mode) => mode.mode).join(" ")}`,
				).includes(needle)),
	);
	const family = families.find((item) => item.key === familyKey) ?? null;
	const definition =
		family?.modes.find((mode) => fixtureDefinitionKey(mode) === definitionKey) ??
		family?.modes[0] ??
		null;
	return (
		<section className="csv-import-picker" aria-label="Library fixture">
			<header>
				<div>
					<h3>{describeSourceType(source)}</h3>
					<p className="patch-secondary">
						{source.rowCount} row{source.rowCount === 1 ? "" : "s"} in the
						file.{" "}
						{current === "skip"
							? "These rows are skipped."
							: "Choose the library fixture and mode these rows use."}
					</p>
				</div>
				<TextInput
					clearable
					aria-label="Search library fixtures"
					placeholder="Search manufacturer, fixture, mode, or type"
					value={query}
					onChange={(event) => setQuery(event.target.value)}
				/>
			</header>
			<div className="fixture-picker-columns">
				<section>
					<h3>Manufacturer</h3>
					<Button
						className={!manufacturer ? "active" : ""}
						onClick={() => setManufacturer("")}
					>
						<span>All manufacturers</span>
					</Button>
					{manufacturers.map((name) => (
						<Button
							key={name}
							className={manufacturer === name ? "active" : ""}
							onClick={() => setManufacturer(name)}
						>
							<span>{name}</span>
						</Button>
					))}
				</section>
				<section>
					<h3>Fixture</h3>
					{filtered.map((item) => (
						<Button
							key={item.key}
							className={familyKey === item.key ? "active" : ""}
							onClick={() => {
								setFamilyKey(item.key);
								const mode =
									item.modes.find(
										(candidate) =>
											normalizeName(candidate.mode) ===
											normalizeName(source.mode),
									) ?? item.modes[0];
								setDefinitionKey(mode ? fixtureDefinitionKey(mode) : "");
							}}
						>
							<span>{item.name}</span>
							<small>
								{item.manufacturer} · {item.modes.length} modes
							</small>
						</Button>
					))}
					{!filtered.length && <p>No library fixture matches the search.</p>}
				</section>
				<section className="fixture-mode-detail">
					{family && definition ? (
						<>
							<h3>
								{family.manufacturer} {family.name}
							</h3>
							<FixtureModeSelect
								modes={family.modes}
								value={fixtureDefinitionKey(definition)}
								onChange={setDefinitionKey}
							/>
							<FixtureDetails definition={definition} />
						</>
					) : (
						<p>Select a fixture.</p>
					)}
					<div className="csv-import-picker-actions">
						<Button onClick={onSkip}>Skip these rows</Button>
						<Button
							className="primary"
							disabled={!definition}
							onClick={() => definition && onUse(definition)}
						>
							Use this fixture
						</Button>
					</div>
				</section>
			</div>
		</section>
	);
}

const STATUS_LABELS: Record<CsvImportRowPlan["status"], string> = {
	ready: "Ready",
	unpatched: "Unpatched",
	skipped: "Skipped",
	error: "Not imported",
};

function ReviewStep({
	plans,
	layerName,
	conflictPolicy,
	importError,
	onConflictPolicy,
}: {
	plans: readonly CsvImportRowPlan[];
	layerName: string;
	conflictPolicy: CsvConflictPolicy;
	importError: string;
	onConflictPolicy: (value: CsvConflictPolicy) => void;
}) {
	const count = (status: CsvImportRowPlan["status"]) =>
		plans.filter((plan) => plan.status === status).length;
	const metres = (millimetres: number) => `${(millimetres / 1000).toFixed(3)} m`;
	return (
		<div className="csv-import-body csv-import-review">
			<div className="csv-import-options">
				<p className="csv-import-counts" aria-label="Import summary">
					<span>{count("ready")} ready</span>
					<span>{count("unpatched")} unpatched</span>
					<span>{count("skipped")} skipped</span>
					<span>{count("error")} not imported</span>
					<span>Layer {layerName}</span>
				</p>
				<MultiValueToggleField
					label="Address conflicts"
					value={conflictPolicy}
					options={[
						{ value: "unpatch", label: "Import unpatched" },
						{ value: "skip", label: "Skip row" },
					]}
					onChange={onConflictPolicy}
				/>
			</div>
			{importError && (
				<p className="patch-status" role="alert">
					{importError}
				</p>
			)}
			<div className="csv-import-table-scroll">
				<table className="csv-import-table" aria-label="Fixtures to import">
					<thead>
						<tr>
							<th>Row</th>
							<th>Status</th>
							<th>Fixture ID</th>
							<th>Name</th>
							<th>Library fixture</th>
							<th>Patch</th>
							<th>X</th>
							<th>Y</th>
							<th>Z</th>
							<th>RotX</th>
							<th>RotY</th>
							<th>RotZ</th>
							<th>Note</th>
						</tr>
					</thead>
					<tbody>
						{plans.map((plan) => (
							<tr key={plan.line} className={`csv-import-${plan.status}`}>
								<td>{plan.line}</td>
								<td>{STATUS_LABELS[plan.status]}</td>
								<td>
									{plan.virtualFixtureNumber != null
										? `0.${plan.virtualFixtureNumber}`
										: (plan.fixtureNumber ?? "—")}
								</td>
								<td>{plan.name || "—"}</td>
								<td>{plan.definition ? definitionLabel(plan.definition) : "—"}</td>
								<td>
									{plan.patch
										? `${plan.patch.universe}.${plan.patch.address}`
										: "—"}
								</td>
								<td>{metres(plan.location.x)}</td>
								<td>{metres(plan.location.y)}</td>
								<td>{metres(plan.location.z)}</td>
								<td>{plan.rotation.x}°</td>
								<td>{plan.rotation.y}°</td>
								<td>{plan.rotation.z}°</td>
								<td>{plan.message ?? ""}</td>
							</tr>
						))}
					</tbody>
				</table>
			</div>
		</div>
	);
}
