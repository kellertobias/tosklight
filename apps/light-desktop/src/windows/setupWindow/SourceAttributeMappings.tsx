import { Button, FormLayout, SelectField, TextField } from "@tosklight/ui";
import { useEffect, useState } from "react";
import type { FixtureSourceMapping } from "../../api/client/fixtures";
import { useFixtureLibrary } from "../../features/fixtureLibrary/FixtureLibraryContext";

interface MappingTarget {
	value: string;
	label: string;
}

/** Desk-local aliases from fixture-source attribute names onto desk attributes. */
export function SourceAttributeMappings({
	targets,
}: {
	targets: MappingTarget[];
}) {
	const fixtureLibrary = useFixtureLibrary();
	const [mappings, setMappings] = useState<FixtureSourceMapping[]>([]);
	const [sourceAttribute, setSourceAttribute] = useState("");
	const [targetAttribute, setTargetAttribute] = useState("");
	const [error, setError] = useState<string | null>(null);
	useEffect(() => {
		let active = true;
		void fixtureLibrary
			?.fixtureSourceMappings?.()
			.then((next) => active && setMappings(next))
			.catch(
				(reason) =>
					active &&
					setError(reason instanceof Error ? reason.message : String(reason)),
			);
		return () => {
			active = false;
		};
	}, [fixtureLibrary]);
	if (!fixtureLibrary?.fixtureSourceMappings) return null;
	const setTarget = async (
		sourceFormat: string,
		source: string,
		target: string | null,
	) => {
		try {
			const saved = await fixtureLibrary.rememberFixtureSourceMapping?.({
				sourceFormat,
				sourceAttribute: source,
				targetAttribute: target,
			});
			setMappings((current) => [
				...current.filter(
					(mapping) =>
						mapping.source_format !== sourceFormat ||
						mapping.source_attribute !== source,
				),
				...(saved ? [saved] : []),
			]);
			setError(null);
		} catch (reason) {
			setError(reason instanceof Error ? reason.message : String(reason));
		}
	};
	const create = async () => {
		const trimmed = sourceAttribute.trim();
		if (!trimmed || !targetAttribute) return;
		await setTarget("gdtf", trimmed, targetAttribute);
		setSourceAttribute("");
	};
	return (
		<article>
			<header>
				<b>Imported attribute names</b>
				<small>
					Desk-local, and kept out of the show file. Fixture revisions retain
					their already-resolved mapping.
				</small>
			</header>
			<p className="attribute-registry-note">
				Map a name a GDTF file uses onto the attribute this desk already
				programs — for example a GDTF <code>MediaRank</code> onto{" "}
				<code>media.folder</code>. Use <b>New custom attribute</b> instead when
				the source channel means something the desk does not have yet.
			</p>
			{mappings.length > 0 && (
				<ul className="plain-list attribute-mapping-list">
					{mappings.map((mapping) => (
						<li key={`${mapping.source_format}:${mapping.source_attribute}`}>
							<code>
								{mapping.source_format.toUpperCase()}:{mapping.source_attribute}
							</code>
							<SelectField
								ariaLabel={`Map ${mapping.source_format.toUpperCase()}:${mapping.source_attribute} to existing attribute`}
								value={mapping.target_attribute}
								options={targets}
								onChange={(target) =>
									void setTarget(
										mapping.source_format,
										mapping.source_attribute,
										target,
									)
								}
							/>
							<Button
								onClick={() =>
									void setTarget(
										mapping.source_format,
										mapping.source_attribute,
										null,
									)
								}
							>
								Forget mapping
							</Button>
						</li>
					))}
				</ul>
			)}
			<FormLayout labelPlacement="side">
				<TextField
					label="GDTF attribute name"
					value={sourceAttribute}
					onChange={(event) => setSourceAttribute(event.target.value)}
					description="The attribute name as the GDTF file spells it, without the GDTF: prefix."
				/>
				<SelectField
					label="Means this attribute"
					ariaLabel="Means this attribute"
					value={targetAttribute}
					options={[{ value: "", label: "Choose an attribute" }, ...targets]}
					onChange={setTargetAttribute}
				/>
			</FormLayout>
			<Button
				disabled={!sourceAttribute.trim() || !targetAttribute}
				onClick={() => void create()}
			>
				Map imported name
			</Button>
			{error && <p role="alert">{error}</p>}
		</article>
	);
}
