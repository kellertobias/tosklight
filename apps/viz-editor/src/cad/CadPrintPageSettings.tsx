import { Button } from "@tosklight/ui";
import { withPageViewRotation } from "./printFrameGeometry";
import { printScaleDenominator, withFixedPrintScale } from "./printLayout";
import type { CadPrintPage } from "./types";
import type { CadPrintPages } from "./useCadPrintPages";

/** The selected sheet's identity, exact drawing scale, orientation and removal. */
export function CadPrintPageSettings({ page, printPages }: { page: CadPrintPage; printPages: CadPrintPages }) {
	const plan = page.kind !== "fixture_list";
	const denominator = page.fixedScaleDenominator ?? printScaleDenominator(page);
	return (
		<section className="cad-print-page-settings" aria-label="Selected page settings">
			<label>
				Page title
				<input aria-label="Page title" value={page.name} maxLength={160}
					onChange={(event) => printPages.change(page.id, { name: event.currentTarget.value })} />
			</label>
			<Button onClick={() => printPages.rotate(page.id)}>Rotate page</Button>
			{plan ? <>
				<label className="cad-print-fixed-scale">
					<input type="checkbox" aria-label="Fixed scale" checked={page.fixedScaleDenominator !== undefined}
						onChange={(event) => printPages.change(page.id, withFixedPrintScale(page, event.currentTarget.checked ? denominator : undefined))} />
					Fixed scale
				</label>
				<label>
					Scale 1:
					<input type="number" aria-label="Scale denominator" min={1} max={1_000_000} step={1} value={denominator}
						onChange={(event) => {
							const value = event.currentTarget.valueAsNumber;
							if (Number.isFinite(value) && value >= 1 && value <= 1_000_000) printPages.change(page.id, withFixedPrintScale(page, value));
						}} />
				</label>
				{page.view === "top_down" ? <label>
					View rotation
					<select aria-label="Page view rotation" value={page.rotationQuarterTurns ?? 0}
						onChange={(event) => printPages.change(page.id, withPageViewRotation(page, Number(event.currentTarget.value)))}>
						{[0, 1, 2, 3].map((rotation) => <option key={rotation} value={rotation}>{rotation * 90}°</option>)}
					</select>
				</label> : null}
			</> : null}
			<Button onClick={() => printPages.remove(page.id)}>Delete page</Button>
		</section>
	);
}
