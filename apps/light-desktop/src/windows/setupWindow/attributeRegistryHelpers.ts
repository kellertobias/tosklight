import type {
	AttributeConfiguration,
	AttributeEncoderGroup,
} from "../../api/client/attributeConfiguration";

export function customAttributeId(label: string) {
	const slug =
		label
			.toLowerCase()
			.replace(/[^a-z0-9]+/g, ".")
			.replace(/^\.+|\.+$/g, "") || "attribute";
	return `custom.${slug}.${crypto.randomUUID()}`;
}

export function nextPlacement(
	configuration: AttributeConfiguration,
	group: AttributeEncoderGroup,
) {
	const occupied = new Set(
		configuration.placements
			.filter((placement) => placement.encoder_group === group)
			.map(
				(placement) => `${placement.encoder_page}:${placement.encoder_slot}`,
			),
	);
	for (let page = 1; ; page += 1)
		for (let slot = 1; slot <= 6; slot += 1)
			if (!occupied.has(`${page}:${slot}`))
				return { encoder_page: page, encoder_slot: slot };
}
