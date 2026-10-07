// The operator's contact data reaches the public site only through the
// TOSKLIGHT_IMPRINT_CONTACT environment variable, which CI fills from the repository secret of the
// same name. Nothing personal is committed; every error names keys, never values, so a failing
// build cannot print the secret into a public log.

export const IMPRINT_CONTACT_ENV = "TOSKLIGHT_IMPRINT_CONTACT";

export const IMPRINT_CONTACT_KEYS = Object.freeze([
	"operator",
	"name",
	"street",
	"postalCode",
	"city",
	"country",
	"email",
	"phone",
	"responsibleForContent",
]);

export class ImprintContactError extends Error {
	constructor(message) {
		super(
			`${message}\n` +
				`${IMPRINT_CONTACT_ENV} must be a JSON object with the non-empty string keys ` +
				`${IMPRINT_CONTACT_KEYS.join(", ")}. CI reads it from the repository secret of the same ` +
				`name; local builds can use tools/site-legal/fixtures/imprint-contact.example.json.`,
		);
		this.name = "ImprintContactError";
	}
}

/**
 * Parse and validate the raw secret. Returns a frozen object holding exactly the documented keys,
 * each trimmed. Unknown keys are reported by name (so a renamed field is noticed) but ignored.
 */
export function parseImprintContact(raw) {
	if (typeof raw !== "string" || raw.trim() === "") {
		throw new ImprintContactError(`${IMPRINT_CONTACT_ENV} is not set or empty.`);
	}
	let value;
	try {
		value = JSON.parse(raw);
	} catch {
		// The parser's message can quote part of the input, so it is deliberately not repeated.
		throw new ImprintContactError(`${IMPRINT_CONTACT_ENV} is not valid JSON.`);
	}
	if (value === null || typeof value !== "object" || Array.isArray(value)) {
		throw new ImprintContactError(`${IMPRINT_CONTACT_ENV} must be a JSON object.`);
	}
	const missing = [];
	const contact = {};
	for (const key of IMPRINT_CONTACT_KEYS) {
		const field = value[key];
		if (typeof field !== "string" || field.trim() === "") missing.push(key);
		else contact[key] = field.trim().replace(/\s+/gu, " ");
	}
	if (missing.length) {
		throw new ImprintContactError(
			`${IMPRINT_CONTACT_ENV} is missing or has empty values for: ${missing.join(", ")}.`,
		);
	}
	if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/u.test(contact.email)) {
		throw new ImprintContactError(`${IMPRINT_CONTACT_ENV}.email is not an email address.`);
	}
	if (!/^\+?[\d\s()/-]{6,}$/u.test(contact.phone)) {
		throw new ImprintContactError(`${IMPRINT_CONTACT_ENV}.phone is not a telephone number.`);
	}
	const unknown = Object.keys(value).filter((key) => !IMPRINT_CONTACT_KEYS.includes(key));
	return { contact: Object.freeze(contact), unknownKeys: unknown };
}

export function readImprintContactFromEnvironment(environment = process.env) {
	return parseImprintContact(environment[IMPRINT_CONTACT_ENV]);
}
