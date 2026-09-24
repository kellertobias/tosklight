import { useEffect, useState } from "react";
import type {
	ColorIntentHeadReport,
	ColorResolutionQuality,
} from "../../../api/client/attributeConfiguration";
import { useAttributeConfigurationActions } from "../../../features/attributeConfiguration/AttributeConfigurationActions";

const QUALITY_LABELS: Record<ColorResolutionQuality, string> = {
	exact: "Exact",
	approximate: "Approximate",
	out_of_gamut: "Out of gamut",
	wheel_limited: "Wheel-limited",
	uncalibrated: "Uncalibrated",
	unsupported: "Unsupported",
};

const QUALITY_HINTS: Record<ColorResolutionQuality, string> = {
	exact: "shows this colour exactly",
	approximate: "shows it closely; its profile data is not measured",
	out_of_gamut: "cannot reach it and shows the nearest colour it can make",
	wheel_limited: "can only choose the nearest colour-wheel slot",
	uncalibrated: "has no colour calibration; the colour is a best guess",
	unsupported: "has no colour engine and stays as it is",
};

/** Wait this long after the last colour change before asking how the fixtures took it. */
const SETTLE_MILLIS = 300;

/**
 * How faithfully each selected fixture shows the Color Intent. Approximate, out-of-gamut,
 * wheel-limited, uncalibrated, and unsupported heads are listed by name so none of them is
 * mistaken for an exact match.
 */
export function ColorIntentDiagnostics({
	fixtureIds,
	refreshKey,
}: {
	fixtureIds: readonly string[];
	refreshKey: string;
}) {
	const actions = useAttributeConfigurationActions();
	const [heads, setHeads] = useState<ColorIntentHeadReport[] | null>(null);
	const [error, setError] = useState<string | null>(null);
	const idsKey = fixtureIds.join(",");
	useEffect(() => {
		if (!actions || !idsKey) {
			setHeads(null);
			return;
		}
		let active = true;
		const timer = setTimeout(() => {
			actions
				.colorIntentReport(idsKey.split(","))
				.then((report) => {
					if (!active) return;
					setHeads(report.heads);
					setError(null);
				})
				.catch((reason) => {
					if (!active) return;
					setError(reason instanceof Error ? reason.message : String(reason));
				});
		}, SETTLE_MILLIS);
		return () => {
			active = false;
			clearTimeout(timer);
		};
	}, [actions, idsKey, refreshKey]);
	if (!actions || !idsKey) return null;
	const flagged = (heads ?? []).filter((head) => head.quality !== "exact");
	const headsPerFixture = new Map<string, number>();
	for (const head of heads ?? [])
		headsPerFixture.set(
			head.fixture_id,
			(headsPerFixture.get(head.fixture_id) ?? 0) + 1,
		);
	return (
		<section
			className="color-intent-diagnostics"
			aria-label="Color Intent results"
		>
			<b>Color Intent</b>
			{error && <p role="alert">{error}</p>}
			{!heads && !error && <p role="status">Checking the selected fixtures…</p>}
			{heads && flagged.length === 0 && (
				<p role="status">Every selected fixture shows this colour exactly.</p>
			)}
			{flagged.length > 0 && (
				<ul>
					{flagged.map((head) => (
						<li
							key={`${head.owner_id}:${head.head_name}`}
							data-quality={head.quality}
						>
							<span className="color-intent-quality">
								{QUALITY_LABELS[head.quality]}
							</span>{" "}
							{headLabel(head, (headsPerFixture.get(head.fixture_id) ?? 1) > 1)}{" "}
							{QUALITY_HINTS[head.quality]}
							{head.delta_uv != null && head.quality !== "unsupported"
								? ` (Δu′v′ ${head.delta_uv.toFixed(3)})`
								: ""}
						</li>
					))}
				</ul>
			)}
		</section>
	);
}

function headLabel(head: ColorIntentHeadReport, namesHeads: boolean) {
	const fixture =
		head.fixture_number != null
			? `Fixture ${head.fixture_number}`
			: head.fixture_name || "Fixture";
	return namesHeads && head.head_name ? `${fixture} ${head.head_name}` : fixture;
}
