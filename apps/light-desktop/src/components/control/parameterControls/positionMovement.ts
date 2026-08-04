type AuthoredMovementRepresentation = "speed" | "time" | "speed_or_time";

type MovementParameter = {
	attribute: string;
	metadata?: {
		position_movement_representation?: AuthoredMovementRepresentation;
	};
};

type MovementFixture = {
	definition: {
		heads?: Array<{ parameters: MovementParameter[] }>;
	};
};

export type PositionMovementRepresentation =
	| "speed"
	| "move_time"
	| "speed_time"
	| "mixed"
	| "movement";

function authoredRepresentation(
	representation: AuthoredMovementRepresentation | undefined,
) {
	switch (representation) {
		case "speed":
			return "speed" as const;
		case "time":
			return "move_time" as const;
		case "speed_or_time":
			return "speed_time" as const;
		default:
			return "movement" as const;
	}
}

export function positionMovementRepresentation(
	fixtures: readonly MovementFixture[],
): PositionMovementRepresentation {
	const representations = new Set(
		fixtures.flatMap((fixture) =>
			(fixture.definition.heads ?? []).flatMap((head) =>
				head.parameters
					.filter((parameter) => parameter.attribute === "position.movement")
					.map((parameter) =>
						authoredRepresentation(
							parameter.metadata?.position_movement_representation,
						),
					),
			),
		),
	);
	if (representations.size === 0) return "movement";
	if (representations.size > 1) return "mixed";
	return [...representations][0] ?? "movement";
}

export function formatPositionMovement(
	normalized: string,
	representation: PositionMovementRepresentation,
) {
	const suffix = {
		speed: "speed",
		move_time: "move time",
		speed_time: "speed / time",
		mixed: "mixed representation",
		movement: "movement",
	}[representation];
	return `${normalized} ${suffix}`;
}
