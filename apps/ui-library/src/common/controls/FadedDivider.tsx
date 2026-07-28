export interface FadedDividerProps {
	orientation?: "horizontal" | "vertical";
	className?: string;
}

export function FadedDivider({
	orientation = "horizontal",
	className = "",
}: FadedDividerProps) {
	return (
		<span
			className={`ui-faded-divider is-${orientation} ${className}`.trim()}
			aria-hidden="true"
		/>
	);
}
