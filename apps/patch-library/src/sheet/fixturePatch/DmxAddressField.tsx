import { Button } from "@tosklight/ui";

type DmxAddressFieldProps = {
	label: string;
	value: string;
	details?: string;
	disabled?: boolean;
	onOpen: () => void;
};

/**
 * The shared touch-facing address field. It deliberately behaves like a single
 * input surface while delegating the 512-slot placement workflow to a modal.
 */
export function DmxAddressField({
	label,
	value,
	details,
	disabled,
	onOpen,
}: DmxAddressFieldProps) {
	return (
		<div className="dmx-address-field">
			<span>{label}</span>
			<Button
				className="dmx-address-field-button"
				disabled={disabled}
				aria-label={`${label}, ${value || "unpatched"}`}
				onClick={onOpen}
			>
				<strong>{value || "Unpatched"}</strong>
				{details && <small>{details}</small>}
				<span aria-hidden="true">›</span>
			</Button>
		</div>
	);
}
