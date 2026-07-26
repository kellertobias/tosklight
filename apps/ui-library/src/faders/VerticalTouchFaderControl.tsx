import type { CSSProperties, LabelHTMLAttributes, ReactNode } from "react";

export interface VerticalTouchFaderControlProps
	extends Omit<LabelHTMLAttributes<HTMLLabelElement>, "children" | "style"> {
	label: ReactNode;
	display: ReactNode;
	fraction: number;
	accentColor?: string;
	mode?: ReactNode;
	disabled?: boolean;
	children: ReactNode;
}

/** Shared buttonless face used by vertical touch-fader surfaces. */
export function VerticalTouchFaderControl({
	label,
	display,
	fraction,
	accentColor,
	mode,
	disabled = false,
	className = "",
	children,
	...labelProps
}: VerticalTouchFaderControlProps) {
	return (
		// biome-ignore lint/a11y/noLabelWithoutControl: Callers provide the range input as children.
		<label
			{...labelProps}
			className={`vertical-touch-fader ${disabled ? "disabled" : ""} ${className}`.trim()}
			style={
				{
					"--fader-level": Math.max(0, Math.min(1, fraction)),
					"--fader-color": accentColor ?? "#176777",
					"--fader-color-dark": accentColor
						? `color-mix(in srgb, ${accentColor} 42%, #081014)`
						: "#103039",
				} as CSSProperties
			}
		>
			<span>
				{label}
				{mode && <small>{mode}</small>}
			</span>
			<strong>{display}</strong>
			{children}
		</label>
	);
}
