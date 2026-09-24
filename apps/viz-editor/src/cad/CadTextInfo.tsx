/**
 * Info for picked text: its words, how tall they read, and where they stand.
 *
 * The position is the text's anchor on the view it was placed on — X and Y on a plan, the page's
 * horizontal and height on an elevation — in metres, as every position in Info is. Each field is
 * written when it is committed, as one step Undo puts back, so typing a position moves the text
 * exactly as dragging it does.
 */
import type { CadAnnotation } from "./annotations";
import { CommitNumber, CommitText } from "./cadFields";
import { CAD_FONTS } from "./cadFonts";
import { useCadTools } from "./cadTools";

export function CadTextInfo({ annotation }: { annotation: CadAnnotation }) {
	const tools = useCadTools();
	const [x, y] = annotation.points[0] ?? [0, 0];
	const plan = annotation.view === "top_down";
	const write = (change: Partial<CadAnnotation>) => void tools.change({ ...annotation, ...change });
	const place = (point: [number, number]) =>
		write({
			points: [
				[Math.round(point[0]), Math.round(point[1])],
				...annotation.points.slice(1),
			],
		});
	// Held in from the sidebar's edges exactly as an element's Info is, under the same title row.
	return (
		<div className="cad-sidebar-info">
		<section className="cad-info" aria-label="Info">
			<header className="cad-info-header">
				<h3>Text</h3>
			</header>
			<CommitText
				label="Text"
				value={annotation.text}
				accepts={(draft) => draft.trim() !== ""}
				onCommit={(text) => write({ text: text.trim() })}
			/>
			<CommitNumber
				label="Height"
				unit="m"
				min={0.01}
				max={20}
				value={annotation.textHeightMillimetres / 1000}
				onCommit={(metres) => write({ textHeightMillimetres: Math.round(metres * 1000) })}
			/>
			<label className="cad-field">
				<span>Font</span>
				<select
					className="ui-input"
					aria-label="Font"
					value={CAD_FONTS.some((font) => font.id === (annotation.font ?? "")) ? (annotation.font ?? "") : ""}
					onChange={(event) => write({ font: event.currentTarget.value })}
				>
					{CAD_FONTS.map((font) => (
						<option key={font.id} value={font.id} style={font.family ? { fontFamily: font.family } : undefined}>
							{font.label}
						</option>
					))}
				</select>
			</label>
			<div className="cad-info-vector" role="group" aria-label="Position">
				<span>Position</span>
				<CommitNumber
					label={plan ? "X" : "Across"}
					unit="m"
					value={x / 1000}
					onCommit={(metres) => place([metres * 1000, y])}
				/>
				<CommitNumber
					label={plan ? "Y" : "Height"}
					ariaLabel={plan ? "Y" : "Position height"}
					unit="m"
					value={y / 1000}
					onCommit={(metres) => place([x, metres * 1000])}
				/>
			</div>
		</section>
		</div>
	);
}
