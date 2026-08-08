// Typed models over the catalog projection.
//
// The server owns what an address means; these helpers only answer presentation questions the
// server was never asked — "what should this row say", "is this folder empty".

import type { CatalogFolderView, CatalogItemView, CatalogView } from "../shared/api/generated/media-wire";

export interface ResolvedAddress {
	folder: CatalogFolderView | undefined;
	item: CatalogItemView | undefined;
}

export function resolveAddress(
	catalog: CatalogView | undefined,
	folder: number,
	file: number,
): ResolvedAddress {
	const found = catalog?.folders.find((candidate) => candidate.folder === folder);
	return { folder: found, item: found?.items.find((candidate) => candidate.file === file) };
}

export function folderLabel(folder: CatalogFolderView): string {
	return folder.name ? `${pad(folder.folder)} · ${folder.name}` : pad(folder.folder);
}

export function itemLabel(item: CatalogItemView): string {
	return `${pad(item.file)} · ${item.name}`;
}

/** What an item's shape and length are, in one line, for a table cell. */
export function itemDetail(item: CatalogItemView): string {
	const size = `${item.width}×${item.height}`;
	if (item.kind === "image") return `still · ${size}`;
	const frames = item.frames === null ? "unknown length" : `${item.frames} frames`;
	return `video · ${size} · ${frames}`;
}

/** A DMX address as an operator reads it on a desk: three digits, slash, three digits. */
export function addressLabel(folder: number, file: number): string {
	return `${pad(folder)}/${pad(file)}`;
}

function pad(value: number): string {
	return value.toString().padStart(3, "0");
}

export function itemCount(catalog: CatalogView | undefined): number {
	return catalog?.itemCount ?? 0;
}
