import appMark from "../../../src-tauri/icons/mark-shadow.svg";

export function LoadingSurface({
	title,
	detail,
	note,
	showMark = false,
	className = "",
}: {
	title: string;
	detail: string;
	note?: string;
	showMark?: boolean;
	className?: string;
}) {
	return (
		<div
			className={`desk-loading-surface ${className}`.trim()}
			role="status"
			aria-live="polite"
			aria-busy="true"
		>
			<div className="desk-loading-card">
				{showMark && (
					<div className="app-mark" role="img" aria-label="ToskLight application">
						<img src={appMark} alt="" />
					</div>
				)}
				<span className="status-pulse" aria-hidden="true" />
				<h1>{title}</h1>
				<p>{detail}</p>
				{note && <small>{note}</small>}
			</div>
		</div>
	);
}
