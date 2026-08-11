import type { DeskStateDiagnostic } from "../../../features/deskState/deskStateDiagnostics";

export function DeskStatePanel({
	diagnostics,
}: {
	diagnostics: readonly DeskStateDiagnostic[];
}) {
	if (!diagnostics.length) {
		return (
			<section className="desk-state-panel" aria-label="Desk state diagnostics">
				<div className="desk-state-healthy" role="status">
					<strong>No desk errors</strong>
					<span>Output and desk authorities have no current global fault.</span>
				</div>
			</section>
		);
	}

	return (
		<section className="desk-state-panel" aria-label="Desk state diagnostics">
			<header>
				<strong>
					{diagnostics.length} current desk {diagnostics.length === 1 ? "error" : "errors"}
				</strong>
				<span>Select the conflict below for its operator explanation.</span>
			</header>
			<nav aria-label="Current desk errors">
				{diagnostics.map((diagnostic) => (
					<a key={diagnostic.id} href={`#${diagnostic.id}`}>
						{diagnostic.title}
					</a>
				))}
			</nav>
			<div className="desk-state-error-list">
				{diagnostics.map((diagnostic) => (
					<article key={diagnostic.id} id={diagnostic.id} className="desk-state-error">
						<h3>{diagnostic.title}</h3>
						<p>{diagnostic.summary}</p>
						<p className="desk-state-action">
							<strong>What to do</strong>
							<span>{diagnostic.action}</span>
						</p>
						{diagnostic.detail && <pre>{diagnostic.detail}</pre>}
					</article>
				))}
			</div>
		</section>
	);
}
