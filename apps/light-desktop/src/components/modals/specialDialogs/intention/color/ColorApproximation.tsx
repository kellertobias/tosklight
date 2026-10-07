import type { ColorIntentReport } from "../../../../../api/client/attributeConfiguration";
import {
	acceptedHeads,
	headColorDetail,
	VISIBLE_LABEL,
} from "../../../../../features/colorReport/acceptedColorReport";
import type { ValueRange } from "../HorizontalRangeFader";
import { cssRgb, hueRangeSamples, hueSaturationRgb } from "./colorDialogModel";

export interface ColorApproximationProps {
	/** The requested colour shown once, as CSS. */
	requested: string;
	/** Requested hue range and the saturation it is shown at, for the requested strip. */
	hueRange?: ValueRange;
	saturation: number;
	/** Requested UV in percent, or `null`. */
	uvRequested: number | null;
	report: ColorIntentReport | null;
	/** A fixture whose details were asked for from the Fixture Sheet. */
	focusFixtureId?: string | null;
}

/** The requested hue range sampled along its shortest arc (presentation of the request). */
export function requestedRangeGradient(range: ValueRange, saturation: number) {
	const stops = hueRangeSamples(range, 7).map((hue) =>
		cssRgb(hueSaturationRgb(hue, saturation)),
	);
	return `linear-gradient(90deg, ${stops.join(", ")})`;
}

/**
 * Passive per-fixture approximation of the expanded Color modal. Rows come only from the
 * accepted-frame colour report; nothing is refitted here. The visible match and UV are separate
 * columns. No alert, status or live region: updates never interrupt or move focus.
 */
export function ColorApproximation({
	requested,
	hueRange,
	saturation,
	uvRequested,
	report,
	focusFixtureId,
}: ColorApproximationProps) {
	const heads = acceptedHeads(report);
	const details = (heads ?? []).map(headColorDetail);
	const differing = details.filter((detail) => detail.quality !== "exact");
	return (
		<div className="color-approximation" data-testid="color-approximation">
			<div className="color-approximation-requested">
				<span
					className="color-approximation-swatch"
					data-testid="color-requested-swatch"
					style={{ background: requested }}
				/>
				{hueRange && (
					<span
						className="color-approximation-range"
						data-testid="color-requested-range"
						style={{ background: requestedRangeGradient(hueRange, saturation) }}
					/>
				)}
				<span>
					Requested
					{uvRequested == null ? "" : ` · UV ${Math.round(uvRequested)}%`}
				</span>
			</div>
			{heads === null ? (
				<p className="color-approximation-summary">
					Results appear once this colour is output.
				</p>
			) : (
				<p className="color-approximation-summary">
					{differing.length === 0
						? "Every selected fixture shows this colour exactly."
						: `${differing.length} of ${details.length} heads show it approximately.`}
				</p>
			)}
			{details.length > 0 && (
				<table className="color-approximation-table">
					<thead>
						<tr>
							<th scope="col">Fixture</th>
							<th scope="col">Visible color</th>
							<th scope="col">UV</th>
						</tr>
					</thead>
					<tbody>
						{details.map((detail) => (
							<tr
								key={`${detail.fixtureId}/${detail.ownerId}/${detail.name}`}
								data-fixture-id={detail.fixtureId}
								data-focused={
									focusFixtureId &&
									(focusFixtureId === detail.fixtureId ||
										focusFixtureId === detail.ownerId)
										? "true"
										: undefined
								}
							>
								<th scope="row">{detail.name}</th>
								<td data-quality={detail.quality}>
									<b>{VISIBLE_LABEL[detail.quality]}</b> {detail.visible}
									{detail.deltaUv != null &&
										detail.quality !== "exact" &&
										` · Δu′v′ ${detail.deltaUv.toFixed(4)}`}
									{detail.note && (
										<span className="color-approximation-note">
											{` · ${detail.note}`}
										</span>
									)}
								</td>
								<td>{detail.uv ?? (uvRequested == null ? "—" : "Not reported")}</td>
							</tr>
						))}
					</tbody>
				</table>
			)}
		</div>
	);
}
