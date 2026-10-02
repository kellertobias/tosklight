import type { ColorIntentHeadReport } from "../../../../../api/client/attributeConfiguration";
import type {
	ColorIntentDirectReport,
	NativeColorPagesSnapshot,
} from "../../../../../api/nativeColorModels";

/**
 * TL-554 passive wording of the Direct (native) Color status. An exact native replay is native
 * identity only: it never reads "exact colour" (the visible match is the report's own column).
 * An unknown appearance is "native only", never a guessed colour; UV is described on its own.
 */

const REPLAY: Readonly<Record<ColorIntentDirectReport["replay"], string>> = {
	exact: "Native replay",
	fallback: "Best-effort match",
	native_only: "Native only · appearance unknown",
};

const REASON: Readonly<
	Record<NonNullable<ColorIntentDirectReport["compatibility"]>, string>
> = {
	compatible: "compatible layout",
	no_native_color: "no native colour",
	different_source: "different fixture type",
	changed_layout: "changed native layout",
	unknown: "model unavailable",
};

export interface DirectStatusRow {
	key: string;
	name: string;
	replay: string;
	detail: string[];
}

export function headName(head: {
	fixture_number?: number | null;
	fixture_name: string;
	head_name?: string;
}) {
	return [
		head.fixture_number == null ? null : String(head.fixture_number),
		head.fixture_name || null,
		head.head_name || null,
	]
		.filter(Boolean)
		.join(" · ");
}

export function directStatusRow(head: ColorIntentHeadReport): DirectStatusRow | null {
	const direct = head.direct;
	if (!direct) return null;
	const detail: string[] = [];
	if (direct.compatibility && direct.compatibility !== "compatible")
		detail.push(REASON[direct.compatibility]);
	if (direct.uv === "apply") detail.push("UV applied");
	if (direct.uv === "park_off") detail.push("UV parked off");
	if (direct.origin === "recorded") detail.push("recorded estimate");
	if (direct.drive_limit === "above_model_maximum") detail.push("above model maximum");
	detail.push(...direct.limitations);
	return {
		key: `${head.fixture_id}:${head.owner_id}:${head.head_name}`,
		name: headName(head),
		replay: REPLAY[direct.replay],
		detail,
	};
}

/** How a selected fixture would replay the reference recipe, before any Direct value exists. */
export function replayPreviewText(
	pages: NativeColorPagesSnapshot | null,
	fixtureId: string,
): string | null {
	const preview = pages?.fixtures.find((entry) => entry.fixture_id === fixtureId);
	if (!preview) return null;
	return preview.replay === "exact" ? "Replays exactly" : "Best-effort match";
}

/** The reference head, clearly identified. */
export function referenceLabel(pages: NativeColorPagesSnapshot | null) {
	const reference = pages?.reference;
	return reference ? headName(reference) : null;
}
