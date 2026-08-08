// The library, as the desk addresses it.
//
// The address is the point of this page: an operator reading a cue sheet needs to see which
// folder and file number reaches which clip, so the DMX address is the first column and the file
// name is a detail beside it.

import { SearchBar } from "@tosklight/ui/controls";
import { useMemo, useState } from "react";
import { ResourceState } from "../../app/ResourceState";
import { addressLabel, folderLabel, itemDetail } from "../../entities/catalog";
import type { CatalogView } from "../../shared/api/generated/media-wire";
import { useCatalog } from "../../shared/api/queries";

const CATALOG_POLL_MS = 15_000;

export function LibraryPage() {
	const catalog = useCatalog(CATALOG_POLL_MS);
	const [search, setSearch] = useState("");

	return (
		<section className="media-page">
			<SearchBar
				value={search}
				onChange={setSearch}
				ariaLabel="Search the library"
				placeholder="Search the library"
			/>
			<ResourceState
				resource={catalog}
				subject="the library"
				isEmpty={(data) => data.itemCount === 0}
				empty="The library folder is empty. Import media to fill it."
			>
				{(data) => <Folders catalog={data} search={search} />}
			</ResourceState>
		</section>
	);
}

function Folders({ catalog, search }: { catalog: CatalogView; search: string }) {
	const folders = useMemo(() => matching(catalog, search), [catalog, search]);

	if (folders.length === 0) {
		return <p className="media-state is-empty">Nothing in the library matches “{search}”.</p>;
	}

	return (
		<>
			{folders.map((folder) => (
				<section key={folder.folder} className="media-folder" aria-label={folderLabel(folder)}>
					<h2>{folderLabel(folder)}</h2>
					<table className="media-table">
						<caption className="media-visually-hidden">
							Items in {folderLabel(folder)}
						</caption>
						<thead>
							<tr>
								<th scope="col">Address</th>
								<th scope="col">Name</th>
								<th scope="col">Detail</th>
								<th scope="col">Tempo</th>
							</tr>
						</thead>
						<tbody>
							{folder.items.map((item) => (
								<tr key={item.id}>
									<td>{addressLabel(folder.folder, item.file)}</td>
									<td>{item.name}</td>
									<td>{itemDetail(item)}</td>
									<td>{item.intrinsicBpm === null ? "—" : `${item.intrinsicBpm} BPM`}</td>
								</tr>
							))}
						</tbody>
					</table>
				</section>
			))}
		</>
	);
}

/** Folders whose name, or one of whose items, matches. An empty search matches everything. */
function matching(catalog: CatalogView, search: string): CatalogView["folders"] {
	const needle = search.trim().toLowerCase();
	if (!needle) return catalog.folders;
	return catalog.folders
		.map((folder) => ({
			...folder,
			items: folder.items.filter((item) => item.name.toLowerCase().includes(needle)),
		}))
		.filter(
			(folder) =>
				folder.items.length > 0 || (folder.name ?? "").toLowerCase().includes(needle),
		);
}
