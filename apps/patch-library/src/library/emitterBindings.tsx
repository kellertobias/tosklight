import { SelectField } from "@tosklight/ui";
import type { FixtureMode, GeometryGraph } from "../wire";

/**
 * Which of the fixture's emitters each of this mode's heads owns.
 *
 * This is the whole of what a personality says about geometry. The parts, the axes and the
 * emitters belong to the lantern and are edited once, under Geometry; a mode only decides which
 * of its heads drives which of them — and an emitter no head owns is not lit in that mode, which
 * is how one personality gives every ring of a wash its own head and another drives them together.
 */
export function EmitterBindings({
	mode,
	geometry,
	onChange,
}: {
	mode: FixtureMode;
	geometry: GeometryGraph;
	onChange: (mode: FixtureMode) => void;
}) {
	if (!geometry.emitters.length) {
		return (
			<p className="empty-editor-message" role="status">
				This fixture has no emitters yet. Add them under <strong>Geometry</strong>,
				which is where the lantern itself is described, and they appear here for
				every mode to drive.
			</p>
		);
	}
	const owner = (emitterId: string) =>
		mode.emitter_heads?.find((binding) => binding.emitter_id === emitterId)
			?.head_id ?? "";
	return (
		<section className="fixture-emitter-bindings">
			<h3>Emitters</h3>
			<p className="field-hint">
				An emitter no head owns is not lit in this mode.
			</p>
			{geometry.emitters.map((emitter) => (
				<SelectField
					key={emitter.id}
					label={emitter.name || "Emitter"}
					value={owner(emitter.id)}
					options={[
						{ value: "", label: "Not driven in this mode" },
						...mode.heads.map((head) => ({
							value: head.id,
							label: head.name,
						})),
					]}
					onChange={(head_id) => {
						const kept = (mode.emitter_heads ?? []).filter(
							(binding) => binding.emitter_id !== emitter.id,
						);
						onChange({
							...mode,
							emitter_heads: head_id
								? [...kept, { emitter_id: emitter.id, head_id }]
								: kept,
						});
					}}
				/>
			))}
		</section>
	);
}
