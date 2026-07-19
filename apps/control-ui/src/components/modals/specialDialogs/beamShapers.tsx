import type { PatchedFixture } from "../../../api/types";
import { Button } from "../../common";
import { VerticalTouchFader } from "../../control/VerticalTouchFader";

type BeamFamily = "Beam" | "Shapers";

interface BeamShapersDialogProps {
	attributes: string[];
	family: BeamFamily;
	page: number;
	apply: (attribute: string, value: number) => Promise<void>;
	setPage: (page: number) => void;
}

export function availableSpecialDialogAttributes(
	fixtures: PatchedFixture[],
	selectedFixtureIds: readonly string[],
): Set<string> {
	const result = new Set<string>();
	for (const fixture of fixtures) {
		if (
			selectedFixtureIds.includes(fixture.fixture_id) ||
			fixture.logical_heads.some((head) =>
				selectedFixtureIds.includes(head.fixture_id),
			)
		) {
			for (const head of fixture.definition.heads ?? []) {
				for (const parameter of head.parameters)
					result.add(parameter.attribute);
			}
		}
	}
	return result;
}

export function beamAttributesForFamily(
	available: Set<string>,
	family: BeamFamily,
): string[] {
	return [...available].filter((attribute) =>
		family === "Shapers"
			? attribute.startsWith("shaper.")
			: /^(gobo|prism|iris)/.test(attribute),
	);
}

export function BeamShapersDialog({
	attributes,
	family,
	page,
	apply,
	setPage,
}: BeamShapersDialogProps) {
	const pageAttributes = attributes.slice(page * 4, page * 4 + 4);
	return (
		<div className="beam-pages">
			<header>
				<b>
					{family} page {page + 1}
				</b>
				<span className="spacer" />
				<Button disabled={page === 0} onClick={() => setPage(page - 1)}>
					←
				</Button>
				<Button
					disabled={(page + 1) * 4 >= attributes.length}
					onClick={() => setPage(page + 1)}
				>
					→
				</Button>
			</header>
			<div>
				{pageAttributes.length ? (
					pageAttributes.map((attribute) => (
						<VerticalTouchFader
							key={attribute}
							label={attribute.replaceAll(".", " ")}
							value={0}
							onChange={(value) => void apply(attribute, value / 100)}
						/>
					))
				) : (
					<p>
						No {family.toLowerCase()} attributes exist on the selected fixtures.
					</p>
				)}
			</div>
		</div>
	);
}
