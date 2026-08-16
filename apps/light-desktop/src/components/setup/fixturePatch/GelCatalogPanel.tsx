import { Button, Input, Select, TextField } from "@tosklight/ui";
import { useEffect, useState } from "react";
import type {
	GelCatalog,
	GelCatalogImportPreview,
} from "../../../api/client/fixtures";
import type { GelAssignment } from "../../../api/types";
import type { PatchController } from "./controller";
import {
	gelImportConflict,
	gelImportTarget,
	gelSummary,
	mergeGelCatalogs,
} from "./lightSourceAppearanceModel";

type CatalogGel = Extract<GelAssignment, { type: "built_in" }>;
type CatalogApi = PatchController["server"];

export function GelCatalogPanel({
	active,
	api,
	selectedGel,
	onSelect,
	onError,
}: {
	active: boolean;
	api: CatalogApi;
	selectedGel: CatalogGel | null;
	onSelect: (gel: CatalogGel) => void;
	onError: (message: string) => void;
}) {
	const model = useGelCatalogModel(active, api, onError);
	if (!active) return null;
	const availability = catalogGelAvailability(
		selectedGel,
		model.catalogIndex,
		model.catalogIndexLoaded,
	);
	return (
		<section className="gel-catalog-panel" aria-label="Gel catalog">
			{selectedGel && availability !== "available" && (
				<UnavailableCatalogGel
					gel={selectedGel}
					availability={availability}
					catalog={model.catalogIndex.find(
						(candidate) => candidate.id === selectedGel.catalog_id,
					)}
				/>
			)}
			<TextField
				label="Search catalog, number, or name"
				value={model.query}
				onChange={(event) => model.setQuery(event.target.value)}
			/>
			{selectedGel && (
				<p className="patch-secondary">Selected: {gelSummary(selectedGel)}</p>
			)}
			<div className="gel-catalog-results" aria-busy={model.busy}>
				{model.catalogs.flatMap((catalog) =>
					catalog.entries.map((entry) => (
						<Button
							key={`${catalog.id}:${entry.id}`}
							className="gel-catalog-entry"
							aria-label={`Select ${catalog.name} ${entry.number} ${entry.name}`}
							onClick={() => onSelect(toCatalogGel(catalog, entry))}
						>
							<span
								className="gel-catalog-swatch"
								style={{ backgroundColor: entry.display_srgb }}
								role="img"
								aria-label={`Display color ${entry.display_srgb}`}
							/>
							<span className="gel-catalog-number">{entry.number}</span>
							<span className="gel-catalog-name">{entry.name}</span>
							<span className="gel-catalog-maker">{catalog.name}</span>
						</Button>
					)),
				)}
				{!model.busy &&
					model.catalogs.every((catalog) => !catalog.entries.length) && (
						<p className="patch-secondary">No matching catalog gels.</p>
					)}
			</div>
			<GelCatalogImport model={model} />
		</section>
	);
}

function UnavailableCatalogGel({
	gel,
	availability,
	catalog,
}: {
	gel: CatalogGel;
	availability: Exclude<CatalogGelAvailability, "available">;
	catalog?: GelCatalog;
}) {
	if (availability === "checking")
		return (
			<p className="patch-secondary" role="status">
				Checking installed catalog for {gelSummary(gel)}…
			</p>
		);
	const fallback = gel.embedded_fallback;
	return (
		<section className="gel-catalog-unavailable" aria-label="Unavailable gel">
			<p role="alert">
				<strong>
					{availability === "catalog_unavailable"
						? "Catalog unavailable"
						: "Catalog entry unavailable"}
				</strong>
				. The stored appearance continues to use its embedded fallback.
			</p>
			<p className="patch-secondary">
				Stored reference: {gel.catalog_id} / {gel.entry_id}
				{catalog ? ` in ${catalog.name}` : ""}
			</p>
			<p className="patch-secondary">
				Fallback: {fallback.number} · {fallback.name} · display{" "}
				{fallback.display_srgb} · visualizer {fallback.visualizer_srgb}
			</p>
			<p className="patch-secondary">
				Search for a replacement below or import/update a catalog CSV. Selecting
				a result explicitly reconciles this fixture; closing or applying other
				changes keeps this reference and fallback unchanged.
			</p>
		</section>
	);
}

type CatalogGelAvailability =
	| "checking"
	| "available"
	| "catalog_unavailable"
	| "entry_unavailable";

function catalogGelAvailability(
	gel: CatalogGel | null,
	catalogs: GelCatalog[],
	catalogIndexLoaded: boolean,
): CatalogGelAvailability {
	if (!gel || !catalogIndexLoaded) return "checking";
	const catalog = catalogs.find((candidate) => candidate.id === gel.catalog_id);
	if (!catalog) return "catalog_unavailable";
	return catalog.entries.some((entry) => entry.id === gel.entry_id)
		? "available"
		: "entry_unavailable";
}

function GelCatalogImport({ model }: { model: GelCatalogModel }) {
	return (
		<details className="gel-catalog-import">
			<summary>Import gel catalog CSV</summary>
			{/* biome-ignore lint/a11y/noLabelWithoutControl: Select renders its native control inside this label. */}
			<label>
				Import into
				<Select
					aria-label="Import gel catalog target"
					value={model.importCatalogId}
					onChange={(event) => model.selectImportCatalog(event.target.value)}
				>
					<option value="new">New catalog</option>
					{model.catalogIndex.map((catalog) => (
						<option key={catalog.id} value={catalog.id}>
							{catalog.name} · revision {catalog.revision}
						</option>
					))}
				</Select>
			</label>
			<TextField
				label="Catalog name"
				value={model.importCatalogName}
				onChange={(event) => model.setImportName(event.target.value)}
			/>
			<label htmlFor="gel-catalog-csv">
				Catalog CSV
				<Input
					id="gel-catalog-csv"
					type="file"
					accept=".csv,text/csv"
					onChange={(event) => model.selectFile(event.target.files?.[0])}
				/>
			</label>
			{model.importFileName && (
				<p className="patch-secondary">{model.importFileName}</p>
			)}
			<Button
				disabled={
					!model.importCsv || !model.importCatalogName.trim() || model.busy
				}
				onClick={() => void model.previewImport()}
			>
				Preview import
			</Button>
			{model.importPreview && <GelImportPreview model={model} />}
			{model.importStatus && <p role="status">{model.importStatus}</p>}
		</details>
	);
}

function GelImportPreview({ model }: { model: GelCatalogModel }) {
	const preview = model.importPreview;
	if (!preview) return null;
	return (
		<section
			className="gel-import-preview"
			aria-label="Gel catalog import preview"
		>
			<p>
				{preview.additions.length} additions · {preview.replacements.length}{" "}
				replacements · {preview.unchanged.length} unchanged
			</p>
			{preview.conflicts.map((conflict) => (
				<p key={JSON.stringify(conflict)} role="alert">
					{gelImportConflict(conflict)}
				</p>
			))}
			{preview.invalid_rows.map((error) => (
				<p key={`${error.row}:${error.message}`} role="alert">
					Row {error.row}: {error.message}
				</p>
			))}
			<Button
				className="primary"
				disabled={!preview.confirmable || model.busy}
				onClick={() => void model.confirmImport()}
			>
				Confirm import
			</Button>
		</section>
	);
}

function useGelCatalogModel(
	active: boolean,
	api: CatalogApi,
	onError: (message: string) => void,
) {
	const [query, setQuery] = useState("");
	const [catalogs, setCatalogs] = useState<GelCatalog[]>([]);
	const [catalogIndex, setCatalogIndex] = useState<GelCatalog[]>([]);
	const [catalogIndexLoaded, setCatalogIndexLoaded] = useState(false);
	const [busy, setBusy] = useState(false);
	const [importCatalogId, setImportCatalogId] = useState("new");
	const [newCatalogId] = useState(() => crypto.randomUUID());
	const [importCatalogName, setImportCatalogName] = useState("");
	const [importCsv, setImportCsv] = useState<Uint8Array | null>(null);
	const [importFileName, setImportFileName] = useState("");
	const [importPreview, setImportPreview] =
		useState<GelCatalogImportPreview | null>(null);
	const [importStatus, setImportStatus] = useState("");
	useEffect(() => {
		if (!active || !api?.gelCatalogs) return;
		let current = true;
		setBusy(true);
		onError("");
		const indexRequest =
			query.trim() && !catalogIndexLoaded ? api.gelCatalogs("") : null;
		void Promise.all([indexRequest, api.gelCatalogs(query)])
			.then(([index, next]) => {
				if (!current) return;
				setCatalogs(next);
				if (index || !query.trim()) {
					setCatalogIndex(index ?? next);
					setCatalogIndexLoaded(true);
				}
			})
			.catch((reason) => current && onError(errorMessage(reason)))
			.finally(() => current && setBusy(false));
		return () => {
			current = false;
		};
	}, [active, api, query, catalogIndexLoaded, onError]);
	const target = gelImportTarget(importCatalogId, newCatalogId, catalogIndex);
	const previewImport = async () => {
		if (!api?.previewGelCatalogCsvImport || !importCsv || !target || busy)
			return;
		setBusy(true);
		setImportStatus("");
		onError("");
		try {
			setImportPreview(
				await api.previewGelCatalogCsvImport({
					target,
					catalogName: importCatalogName.trim(),
					csv: importCsv,
				}),
			);
		} catch (reason) {
			onError(errorMessage(reason));
			setImportPreview(null);
		} finally {
			setBusy(false);
		}
	};
	const confirmImport = async () => {
		if (
			!api?.confirmGelCatalogCsvImport ||
			!api.gelCatalogs ||
			!importCsv ||
			!target ||
			!importPreview?.confirmable ||
			busy
		)
			return;
		setBusy(true);
		onError("");
		try {
			const imported = await api.confirmGelCatalogCsvImport({
				target,
				catalogName: importCatalogName.trim(),
				csv: importCsv,
			});
			setCatalogs(await api.gelCatalogs(query));
			setCatalogIndex((known) => mergeGelCatalogs(known, [imported]));
			setImportPreview(null);
			setImportStatus(
				`Imported ${imported.name} revision ${imported.revision}.`,
			);
			setImportCatalogId(imported.id);
		} catch (reason) {
			onError(errorMessage(reason));
		} finally {
			setBusy(false);
		}
	};
	return {
		query,
		setQuery,
		catalogs,
		catalogIndex,
		catalogIndexLoaded,
		busy,
		importCatalogId,
		importCatalogName,
		importCsv,
		importFileName,
		importPreview,
		importStatus,
		previewImport,
		confirmImport,
		selectImportCatalog(id: string) {
			setImportCatalogId(id);
			setImportCatalogName(
				catalogIndex.find((catalog) => catalog.id === id)?.name ?? "",
			);
			setImportPreview(null);
		},
		setImportName(name: string) {
			setImportCatalogName(name);
			setImportPreview(null);
		},
		selectFile(file?: File) {
			setImportPreview(null);
			setImportFileName(file?.name ?? "");
			if (!file) {
				setImportCsv(null);
				return;
			}
			void file
				.arrayBuffer()
				.then((buffer) => setImportCsv(new Uint8Array(buffer)));
		},
	};
}

type GelCatalogModel = ReturnType<typeof useGelCatalogModel>;

function toCatalogGel(
	catalog: GelCatalog,
	entry: GelCatalog["entries"][number],
): CatalogGel {
	return {
		type: "built_in",
		catalog_id: catalog.id,
		entry_id: entry.id,
		embedded_fallback: {
			number: entry.number,
			name: entry.name,
			display_srgb: entry.display_srgb,
			visualizer_srgb: entry.visualizer_srgb,
		},
	};
}

function errorMessage(reason: unknown) {
	return reason instanceof Error ? reason.message : String(reason);
}
