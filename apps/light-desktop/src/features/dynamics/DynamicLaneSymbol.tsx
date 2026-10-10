import type { DynamicLaneProjection } from "../../api/types";
import { dynamicLaneOwner } from "./laneModel";

type LaneFamily = "intensity" | "position" | "color" | "other";
type LaneAttribute = { id: string; family: string };

/** Use registry families for scalar lanes and semantic owners for typed lanes. */
export function dynamicLaneFamilies(
	lanes: readonly DynamicLaneProjection[],
	attributes: readonly LaneAttribute[],
): Set<LaneFamily> {
	const families = new Set<LaneFamily>();
	for (const lane of lanes) {
		const owner = dynamicLaneOwner(lane);
		const family = attributes
			.find((attribute) => attribute.id === owner)
			?.family.toLowerCase();
		if (
			family === "intensity" ||
			owner === "intensity" ||
			owner.startsWith("intensity.")
		)
			families.add("intensity");
		else if (
			family === "position" ||
			owner === "position" ||
			owner.startsWith("position.") ||
			["pan", "tilt", "pan.continuous", "tilt.continuous"].includes(owner)
		)
			families.add("position");
		else if (
			family === "color" ||
			owner === "color" ||
			owner.startsWith("color.")
		)
			families.add("color");
		else families.add("other");
	}
	return families;
}

const sectors = [
	{
		family: "intensity", label: "Intensity", letter: "I",
		points: "1,1 47,1 24,24", x: 24, y: 9,
	},
	{
		family: "position", label: "Position", letter: "P",
		points: "1,1 24,24 1,47", x: 9, y: 24,
	},
	{
		family: "color", label: "Color", letter: "C",
		points: "47,1 47,47 24,24", x: 39, y: 24,
	},
	{
		family: "other", label: "Other", letter: "O",
		points: "1,47 24,24 47,47", x: 24, y: 39,
	},
] as const;

export function DynamicLaneSymbol({
	lanes,
	attributes,
}: {
	lanes: readonly DynamicLaneProjection[];
	attributes: readonly LaneAttribute[];
}) {
	const families = dynamicLaneFamilies(lanes, attributes);
	const present = sectors
		.filter((sector) => families.has(sector.family))
		.map((sector) => sector.label);
	const label = `Lanes: ${present.join(", ") || "none"}`;
	return (
		<svg
			className="dynamic-lane-symbol"
			viewBox="0 0 48 48"
			role="img"
			aria-label={label}
		>
			<title>{label}</title>
			{sectors.map((sector) => {
				const active = families.has(sector.family);
				return (
					<g
						key={sector.family}
						data-lane-family={sector.family}
						data-active={active}
					>
						<polygon
							points={sector.points}
							fill={active ? "var(--pool-card-icon-color, #4edcff)" : "#303943"}
							stroke="#10161c"
							strokeWidth="2"
							strokeLinejoin="round"
						/>
						<text
							x={sector.x}
							y={sector.y}
							textAnchor="middle"
							dominantBaseline="central"
							fill={active ? "#080b0f" : "#a5afb8"}
							fontSize="12"
							fontWeight="800"
							fontFamily="system-ui, sans-serif"
						>
							{sector.letter}
						</text>
					</g>
				);
			})}
		</svg>
	);
}
