/** Preserve the actual time/corner while a coincident touch affordance is displaced. */
export function CuePointLeader({
	showLeader,
	anchorTop,
	top,
	anchorLeft,
	left,
}: {
	showLeader: boolean;
	anchorTop?: number;
	top?: number;
	anchorLeft?: number;
	left: number;
}) {
	if (
		!showLeader ||
		!(
			(anchorTop !== undefined && top !== undefined && anchorTop !== top) ||
			(anchorLeft !== undefined && anchorLeft !== left)
		)
	)
		return null;
	return (
		<svg
			className="timecode-cue-point-leader"
			aria-hidden="true"
			style={{
				left: 22 + Math.min(0, (anchorLeft ?? left) - left),
				top: 22 + Math.min(0, (anchorTop ?? top ?? 0) - (top ?? 0)),
				width: Math.max(1, Math.abs((anchorLeft ?? left) - left)),
				height: Math.max(1, Math.abs((anchorTop ?? top ?? 0) - (top ?? 0))),
			}}
		>
			<line
				x1={Math.max(0, left - (anchorLeft ?? left))}
				y1={Math.max(0, (top ?? 0) - (anchorTop ?? top ?? 0))}
				x2={Math.max(0, (anchorLeft ?? left) - left)}
				y2={Math.max(0, (anchorTop ?? top ?? 0) - (top ?? 0))}
			/>
		</svg>
	);
}
