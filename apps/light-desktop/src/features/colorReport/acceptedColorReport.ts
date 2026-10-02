import type {
	ColorIntentHeadReport,
	ColorIntentReport,
	ColorResolutionQuality,
} from "../../api/client/attributeConfiguration";

/**
 * Calm, passive descriptions of the accepted-frame colour report (TL-550, fed by TL-594 B).
 *
 * Only a report read from the accepted output frame counts: a legacy report (no
 * `accepted_frame`) or one whose frame is not yet available yields nothing, so an unprogrammed
 * fixture is never compared with an invented white. Visible match and UV are described
 * separately. Nothing here is a warning that interrupts work: the Fixture Sheet shows at most
 * one small steady triangle, and the Color modal lists the details on demand.
 */

/** Δu′v′ at or below this is not visible on stage (help: 05-color-intent). */
export const VISIBLE_DELTA_UV = 0.004;

export const VISIBLE_LABEL: Readonly<Record<ColorResolutionQuality, string>> = {
	exact: "Exact",
	approximate: "Approximate",
	out_of_gamut: "Out of gamut",
	wheel_limited: "Wheel-limited",
	uncalibrated: "Uncalibrated",
	unsupported: "Unsupported",
};

const VISIBLE_TEXT: Readonly<Record<ColorResolutionQuality, string>> = {
	exact: "Shows this colour exactly",
	approximate: "Estimated color",
	out_of_gamut: "Shows the nearest colour it can",
	wheel_limited: "Uses the nearest wheel slot",
	uncalibrated: "Estimated color; no colour calibration",
	unsupported: "No colour engine; colour unchanged",
};

export interface HeadColorDetail {
	fixtureId: string;
	ownerId: string;
	name: string;
	quality: ColorResolutionQuality;
	visible: string;
	deltaUv: number | null;
	/** UV result, separate from the visible match; `null` when UV plays no part. */
	uv: string | null;
	/** An expected limitation worth the passive Fixture Sheet triangle. */
	limited: boolean;
	/** TL-552: colour controls parked at neutral, white only, or why there is no colour model. */
	note: string | null;
}

/** The report's heads, or `null` unless it was read from an accepted output frame. */
export function acceptedHeads(
	report: ColorIntentReport | null | undefined,
): readonly ColorIntentHeadReport[] | null {
	return report?.accepted_frame?.state === "accepted" ? report.heads : null;
}

export function uvText(head: ColorIntentHeadReport): string | null {
	const uv = head.uv;
	if (!uv || uv.status === "not_requested") return null;
	if (uv.status === "unsupported") return "UV unavailable on this fixture";
	return uv.clipped ? "UV limited by the emitter" : "UV applied";
}

/** Expected limitations: a visible miss, no calibration or engine, or UV it cannot give. */
export function isExpectedLimitation(head: ColorIntentHeadReport) {
	if (head.uv?.status === "unsupported" || head.uv?.clipped) return true;
	switch (head.quality) {
		case "exact":
			return false;
		case "approximate":
			return (head.delta_uv ?? 0) > VISIBLE_DELTA_UV;
		default:
			return true;
	}
}

export function headColorDetail(head: ColorIntentHeadReport): HeadColorDetail {
	const name = [
		head.fixture_number == null ? null : String(head.fixture_number),
		head.fixture_name || null,
		head.head_name || null,
	]
		.filter(Boolean)
		.join(" · ");
	return {
		fixtureId: head.fixture_id,
		ownerId: head.owner_id,
		name,
		quality: head.quality,
		visible: VISIBLE_TEXT[head.quality],
		deltaUv: head.delta_uv,
		uv: uvText(head),
		limited: isExpectedLimitation(head),
		note: head.note ?? null,
	};
}

export interface FixtureColorNotice {
	/** Accessible label of the triangle. */
	label: string;
	heads: readonly HeadColorDetail[];
}

/**
 * One notice per Fixture Sheet row (fixture or logical head) with an expected limitation.
 * A row is keyed by both its fixture id and the head's owner id.
 */
export function fixtureColorNotices(
	report: ColorIntentReport | null | undefined,
): ReadonlyMap<string, FixtureColorNotice> {
	const heads = acceptedHeads(report);
	const notices = new Map<string, FixtureColorNotice>();
	if (!heads) return notices;
	const grouped = new Map<string, HeadColorDetail[]>();
	for (const head of heads) {
		const detail = headColorDetail(head);
		if (!detail.limited) continue;
		for (const key of new Set([head.fixture_id, head.owner_id])) {
			const list = grouped.get(key) ?? [];
			list.push(detail);
			grouped.set(key, list);
		}
	}
	for (const [key, details] of grouped)
		notices.set(key, {
			label: `Color details: ${details
				.flatMap((detail) => [detail.visible, detail.uv])
				.filter((text, index, all) => text && all.indexOf(text) === index)
				.join("; ")}`,
			heads: details,
		});
	return notices;
}
