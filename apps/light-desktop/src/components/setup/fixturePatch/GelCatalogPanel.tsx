import { Button, Select, TextField } from "@tosklight/ui";
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
	return (
		<section className="gel-catalog-panel" aria-label="Gel catalog">
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
							onClick={() => onSelect(toCatalogGel(catalog, entry))}
						>
							<span
								className="gel-catalog-swatch"
								style={{ backgroundColor: entry.display_srgb }}
								role="img"
								aria-label={`Display color ${entry.display_srgb}`}
							/>
							{catalog.name} · {entry.number} · {entry.name} ·{" "}
							{entry.display_srgb}
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
			<label>
				Catalog CSV
				<input
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
		void api
			.gelCatalogs(query)
			.then((next) => {
				if (!current) return;
				setCatalogs(next);
				setCatalogIndex((known) => mergeGelCatalogs(known, next));
			})
			.catch((reason) => current && onError(errorMessage(reason)))
			.finally(() => current && setBusy(false));
		return () => {
			current = false;
		};
	}, [active, api, query, onError]);
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
