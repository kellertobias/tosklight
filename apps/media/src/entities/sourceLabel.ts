import type {
	AddressView,
	CatalogView,
	TextSlotView,
	VisualizerView,
} from "../shared/api/generated/media-wire";
import { addressLabel, resolveAddress } from "./catalog";

/** A missing catalogue entry is unresolved, not an empty source. */
export function sourceLabel(
	address: AddressView,
	catalog: CatalogView | undefined,
	text: readonly TextSlotView[] | undefined,
	visualizers: readonly VisualizerView[] | undefined,
): string {
	if (address.class === "blank") return "Nothing selected";
	const matches = (entry: { address: AddressView }) =>
		entry.address.folder === address.folder &&
		entry.address.file === address.file;
	const name =
		address.class === "text-bank"
			? text?.find(matches)?.name
			: address.class === "generated-visualizer"
				? visualizers?.find(matches)?.name
				: resolveAddress(catalog, address.folder, address.file).item?.name;
	return name?.trim() ? name : addressLabel(address.folder, address.file);
}
