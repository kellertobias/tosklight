import { Button } from "@tosklight/ui";
import type { FixtureColorNotice } from "../features/colorReport/acceptedColorReport";
import "./fixtureSheetColorNotice.css";

/**
 * The quiet Fixture Sheet Color information or limitation marker. Passive: it never announces itself, moves
 * focus, animates or opens anything on its own. Only a deliberate activation (click, tap,
 * Enter or Space) opens the Color details; it does not select the row.
 */
export function FixtureSheetColorNotice({
	notice,
	onOpen,
}: {
	notice: FixtureColorNotice;
	onOpen(): void;
}) {
	return (
		<Button
			variant="ghost"
			iconOnly
			className="fixture-sheet-color-notice"
			data-tone={notice.kind}
			aria-label={notice.label}
			title={notice.label}
			data-testid="fixture-sheet-color-notice"
			onPointerDown={(event) => event.stopPropagation()}
			onClick={(event) => {
				event.stopPropagation();
				onOpen();
			}}
			onKeyDown={(event) => {
				if (event.key === "Enter" || event.key === " ") event.stopPropagation();
			}}
		>
			<span className={`fixture-sheet-color-notice-glyph ${notice.kind}`} aria-hidden="true">
				{notice.kind === "info" ? "i" : null}
			</span>
		</Button>
	);
}
