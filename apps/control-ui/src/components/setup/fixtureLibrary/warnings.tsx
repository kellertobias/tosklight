import { useFixtureLibrary } from "../../../features/fixtureLibrary/FixtureLibraryContext";

export function FixtureLibraryWarnings() {
	const server = useFixtureLibrary();
	if (!server?.fixtureProfileWarnings.length) return null;
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
