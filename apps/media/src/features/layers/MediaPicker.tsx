// Choosing what a layer plays.
//
// A desk selects a folder and a file as two separate numbers, and so does this: picking a folder
// must not silently change the file, because an operator building a look expects file 007 to stay
// file 007 while they look through folders.

import { SelectField } from "@tosklight/ui/controls";
import type { CatalogView } from "../../shared/api/generated/media-wire";
import { addressLabel, folderLabel, itemLabel } from "../../entities/catalog";

export interface MediaPickerProps {
	catalog: CatalogView | undefined;
	folder: number;
	file: number;
	disabled: boolean;
	onSelect: (change: { folder?: number; file?: number }) => void;
}

const BLANK = "0";

export function MediaPicker({ catalog, folder, file, disabled, onSelect }: MediaPickerProps) {
	const folders = catalog?.folders ?? [];
	const selected = folders.find((candidate) => candidate.folder === folder);

	return (
		<div className="media-picker">
			<SelectField
				label="Folder"
				value={String(folder)}
				disabled={disabled}
				options={[
					{ value: BLANK, label: "— none —" },
					...folders.map((candidate) => ({
						value: String(candidate.folder),
						label: folderLabel(candidate),
					})),
				]}
				onChange={(next) => onSelect({ folder: Number(next) })}
			/>
			<SelectField
				label="File"
				value={String(file)}
				disabled={disabled || !selected}
				options={[
					{ value: BLANK, label: "— none —" },
					...(selected?.items ?? []).map((item) => ({
						value: String(item.file),
						label: itemLabel(item),
					})),
				]}
				onChange={(next) => onSelect({ file: Number(next) })}
			/>
			<p className="media-address">{addressLabel(folder, file)}</p>
		</div>
	);
}
