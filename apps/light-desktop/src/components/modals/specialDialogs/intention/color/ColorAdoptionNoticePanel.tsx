import { Button } from "@tosklight/ui";
import {
	colorAdoptionNotice,
	useColorAdoptionNotice,
} from "../../../../../features/familyEncoders/colorAdoptionNotice";

/** The explicit starting colours the desk offers; black is never white, white is explicit. */
export const EXPLICIT_STARTS = [
	{ label: "Start from black", rgb: [0, 0, 0] as const },
	{ label: "Start from white", rgb: [1, 1, 1] as const },
];

/**
 * TL-554 explicit-starting-value notice. Shown quietly (no alert, toast or focus move) when the
 * first semantic edit of a Direct value with an unknown appearance was held; the operator's
 * choice is used by the next Color edit. A reported adoption says whether the starting value
 * is an approximation of the Direct value or the explicit start.
 */
export function ColorAdoptionNoticePanel({ compact = false }: { compact?: boolean }) {
	const notice = useColorAdoptionNotice();
	if (notice.required)
		return (
			<div className="color-adoption-notice" data-testid="color-explicit-start">
				<p>
					{notice.explicitStart
						? "Starting colour chosen. Turn the control again to apply it."
						: "The Direct colour's appearance is unknown. Choose a starting colour."}
				</p>
				{!notice.explicitStart && (
					<div className="color-adoption-choices">
						{EXPLICIT_STARTS.map((start) => (
							<Button
								key={start.label}
								onClick={() => colorAdoptionNotice.choose(start.rgb)}
							>
								{start.label}
							</Button>
						))}
					</div>
				)}
			</div>
		);
	if (!notice.adoption || compact) return null;
	const explicit = notice.adoption.fixtures.some((entry) => entry.start === "explicit");
	return (
		<p className="color-adoption-report" data-testid="color-adoption-report">
			{explicit
				? "Started from your explicit colour."
				: "Started from an approximation of the Direct colour."}
			{notice.adoption.fixtures.some((entry) => entry.uvUnknown)
				? " UV was unknown and starts off."
				: ""}
		</p>
	);
}
