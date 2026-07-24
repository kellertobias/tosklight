export type ExpectedDMXByte =
	| number
	| { readonly between: readonly [number, number] };

export interface FixtureQualifier {
	readonly head?: number;
	readonly multipatch?: string;
}

export interface FixtureReference {
	readonly kind: "fixture";
	readonly number: number;
	readonly head?: number;
	readonly multipatch?: string;
}

export interface FixtureRangeReference {
	readonly kind: "fixture-range";
	readonly first: number;
	readonly last: number;
}

export type FixtureDMXTarget = FixtureReference | FixtureRangeReference;
export type FixtureDMXExpectation = Readonly<Record<string, ExpectedDMXByte>>;

export function fixture(
	number: number,
	headOrQualifier?: number | FixtureQualifier,
): FixtureReference {
	assertPositiveInteger(number, "Fixture number");
	const qualifier =
		typeof headOrQualifier === "number"
			? { head: headOrQualifier }
			: (headOrQualifier ?? {});
	if (qualifier.head !== undefined)
		assertPositiveInteger(qualifier.head, "Fixture head");
	if (qualifier.multipatch !== undefined && !qualifier.multipatch.trim())
		throw new Error("Fixture multi-patch qualifier must not be empty");
	return Object.freeze({
		kind: "fixture",
		number,
		head: qualifier.head,
		multipatch: qualifier.multipatch?.trim(),
	});
}

export function fixtureRange(
	first: number,
	last: number,
): FixtureRangeReference {
	assertPositiveInteger(first, "First fixture number");
	assertPositiveInteger(last, "Last fixture number");
	return Object.freeze({ kind: "fixture-range", first, last });
}

export function fixtureReferences(
	target: FixtureDMXTarget,
): FixtureReference[] {
	if (target.kind === "fixture") return [target];
	const direction = target.first <= target.last ? 1 : -1;
	return Array.from(
		{ length: Math.abs(target.last - target.first) + 1 },
		(_, index) => fixture(target.first + index * direction),
	);
}

export function validateFixtureDmxExpectations(
	expected: FixtureDMXExpectation,
): Array<[string, ExpectedDMXByte]> {
	const seen = new Set<string>();
	const entries = Object.entries(expected);
	if (entries.length === 0)
		throw new Error(
			"Fixture DMX expectation must contain at least one channel",
		);
	for (const [name, value] of entries) {
		if (!name.trim())
			throw new Error("Fixture DMX channel name must not be empty");
		const normalized = normalizeDmxName(name);
		if (seen.has(normalized))
			throw new Error(
				`Fixture DMX channel "${name}" is specified more than once`,
			);
		seen.add(normalized);
		if (typeof value === "number") assertByte(value, `DMX value for ${name}`);
		else {
			if (!value || !Array.isArray(value.between) || value.between.length !== 2)
				throw new Error(
					`DMX value for ${name} must be a byte or { between: [minimum, maximum] }`,
				);
			assertByte(value.between[0], `DMX minimum for ${name}`);
			assertByte(value.between[1], `DMX maximum for ${name}`);
			if (value.between[0] > value.between[1])
				throw new Error(`DMX range for ${name} must have minimum <= maximum`);
		}
	}
	return entries;
}

export function dmxByteAccepts(
	expected: ExpectedDMXByte,
	actual: number,
): boolean {
	return typeof expected === "number"
		? actual === expected
		: actual >= expected.between[0] && actual <= expected.between[1];
}

export function describeExpectedDmxByte(expected: ExpectedDMXByte): string {
	return typeof expected === "number"
		? String(expected)
		: `${expected.between[0]}..${expected.between[1]} inclusive`;
}

export function displayAttributeName(attribute: string): string {
	const words = attribute.trim().split(".").join(" ");
	return words ? words[0].toUpperCase() + words.slice(1) : "(unnamed)";
}

export function normalizeDmxName(name: string): string {
	return name.trim().toLocaleLowerCase();
}

function assertPositiveInteger(value: number, label: string): void {
	if (!Number.isInteger(value) || value < 1)
		throw new Error(`${label} must be a positive integer`);
}

function assertByte(value: number, label: string): void {
	if (!Number.isInteger(value) || value < 0 || value > 255)
		throw new Error(`${label} must be an integer from 0 through 255`);
}
