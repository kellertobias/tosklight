import { useServer } from "../../../api/ServerContext";

export function FixtureLibraryWarnings() {
	const server = useServer();
	if (!server.fixtureProfileWarnings.length) return null;
	return (
		<section
			className="fixture-migration-warnings"
			role="alert"
			aria-label="Fixture library migration warnings"
		>
			<h3>Fixture library needs attention</h3>
			{server.fixtureProfileWarnings.map((warning) => (
				<p key={warning}>{warning}</p>
			))}
		</section>
	);
}
