import {
	attributeEncoderGroups,
	type AttributeEncoderPlacement,
	projectPushTurnPlacements,
} from "../control/parameterControls/attributeEncoderPages";
import { useScreens } from "../../features/screens/ScreensContext";

/**
 * Read-only view of the semantic encoder layout at the configured width. It belongs to
 * Attributes & encoders, not to the screen that happens to own the control surface.
 */
export function EncoderLayoutPreview() {
	const server = useScreens();
	const configuration = server.screens?.programmer_control_surface;
	if (!configuration) return null;
	const width = configuration.visible_encoders;
	const placements = (server.bootstrap?.attribute_registry ?? []).flatMap(
		(attribute): AttributeEncoderPlacement[] =>
			attribute.encoder_group &&
			attribute.encoder_page != null &&
			attribute.encoder_slot != null &&
			!attribute.retired
				? [
						{
							id: attribute.id,
							label: attribute.label,
							encoder_group: attribute.encoder_group,
							encoder_page: attribute.encoder_page,
							encoder_slot: attribute.encoder_slot,
							push_turn_of: attribute.push_turn_of,
						},
					]
				: [],
	);
	const groups = attributeEncoderGroups(
		projectPushTurnPlacements(placements),
		new Set(placements.map(({ id }) => id)),
		width,
	).filter((group) => group.pages.length);
	return (
		<div
			className="programmer-control-layout-preview"
			aria-label={`${width}-encoder semantic layout preview`}
		>
			<div>
				<b>{width}-position semantic layout</b>
				<small>
					Page boundaries adapt to the visible encoder count; semantic order and
					push-turn companions stay together.
				</small>
			</div>
			{groups.length ? (
				groups.map((group) => (
					<div className="programmer-control-layout-group" key={group.id}>
						<strong>{group.label}</strong>
						<div
							className="programmer-control-layout-slots"
							style={{
								gridTemplateColumns: `repeat(${width}, minmax(0, 1fr))`,
							}}
						>
							{group.pages[0]?.slots.map((attribute, index) => (
								<span key={attribute?.id ?? `empty-${index}`}>
									<small>E{index + 1}</small>
									<b>{attribute?.label ?? "Unassigned"}</b>
									{attribute?.push_turn_label && (
										<small>Push-turn · {attribute.push_turn_label}</small>
									)}
								</span>
							))}
						</div>
						{group.pages.length > 1 && (
							<small>{group.pages.length} pages at this width</small>
						)}
					</div>
				))
			) : (
				<small>The active show has no placed encoder attributes.</small>
			)}
		</div>
	);
}
