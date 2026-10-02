// Deterministic identity, canonical serialization and contract validation for the semantic
// performance workloads (TL-564). Validation is tied to the generated wire JSON Schemas under
// crates/light/contracts/wire/schemas plus the Rust storage rules that the schemas cannot
// express (Angle pair completeness, representation/component compatibility, domains).
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = path.resolve(
	path.dirname(fileURLToPath(import.meta.url)),
	"..",
);

/** Generated wire schemas that define the Dynamic and programming value contracts. */
export const SEMANTIC_CONTRACT_SCHEMAS = Object.freeze({
	dynamicDefinition: Object.freeze({
		file: "crates/light/contracts/wire/schemas/v2-programming/programming-values-snapshot.schema.json",
		definition: "DynamicDefinitionProjection",
	}),
	programmingAttributeValue: Object.freeze({
		file: "crates/light/contracts/wire/schemas/v2-programming/dynamic-update-action-request.schema.json",
		definition: "ProgrammingAttributeValue",
	}),
});
export const GENERATED_WIRE_TYPES =
	"apps/light-desktop/src/api/generated/light-wire.ts";

// ---------------------------------------------------------------------------------------------
// Identity and determinism

const NAMESPACE_DNS = "6ba7b810-9dad-11d1-80b4-00c04fd430c8";

function uuidBytes(uuid) {
	const hex = uuid.replaceAll("-", "");
	if (!/^[0-9a-f]{32}$/iu.test(hex)) throw new Error(`invalid UUID ${uuid}`);
	return Buffer.from(hex, "hex");
}

function formatUuid(bytes) {
	const hex = Buffer.from(bytes).toString("hex");
	return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20, 32)}`;
}

/** RFC 4122 name-based UUID (version 5), byte-compatible with Rust `Uuid::new_v5`. */
export function uuidV5(namespace, name) {
	const hash = createHash("sha1")
		.update(uuidBytes(namespace))
		.update(Buffer.isBuffer(name) ? name : Buffer.from(String(name), "utf8"))
		.digest();
	const bytes = hash.subarray(0, 16);
	bytes[6] = (bytes[6] & 0x0f) | 0x50;
	bytes[8] = (bytes[8] & 0x3f) | 0x80;
	return formatUuid(bytes);
}

/** Root namespace of every workload identity. Changing it changes every derived ID. */
export const SEMANTIC_WORKLOAD_NAMESPACE = uuidV5(
	NAMESPACE_DNS,
	"semantic-performance-workload.tosklight",
);

export function workloadNamespace(seed) {
	return uuidV5(SEMANTIC_WORKLOAD_NAMESPACE, `seed:${normalizeSeed(seed)}`);
}

export function normalizeSeed(seed) {
	if (typeof seed === "number" && Number.isSafeInteger(seed)) return String(seed);
	if (typeof seed === "string" && seed.trim() !== "" && seed.length <= 128)
		return seed.trim();
	throw new Error("workload seed must be a safe integer or a 1-128 character string");
}

/**
 * The persisted automatic Current partner lane of an Angle Dynamic. Mirrors
 * `current_partner` in crates/light/domain/dynamics/src/angle_pair.rs exactly.
 */
export function angleCurrentPartnerLaneId(definitionId, axis) {
	if (axis !== "pan" && axis !== "tilt")
		throw new Error(`Angle partner axis must be pan or tilt, not ${axis}`);
	return uuidV5(definitionId, `tosklight:position-current:${axis}:v1`);
}

/** Seeded sfc32 generator. The seed is hashed so nearby seeds give unrelated streams. */
export function createSeededRandom(seed, stream = "default") {
	const digest = createHash("sha256")
		.update(`tosklight-semantic-workload:${normalizeSeed(seed)}:${stream}`)
		.digest();
	let a = digest.readUInt32LE(0);
	let b = digest.readUInt32LE(4);
	let c = digest.readUInt32LE(8);
	let d = digest.readUInt32LE(12);
	const next = () => {
		a >>>= 0;
		b >>>= 0;
		c >>>= 0;
		d >>>= 0;
		const t = (a + b + d) >>> 0;
		d = (d + 1) >>> 0;
		a = b ^ (b >>> 9);
		b = (c + (c << 3)) >>> 0;
		c = (c << 21) | (c >>> 11);
		c = (c + t) >>> 0;
		return t / 4_294_967_296;
	};
	for (let index = 0; index < 12; index += 1) next();
	return {
		next,
		range: (minimum, maximum) => minimum + (maximum - minimum) * next(),
		integer: (minimum, maximum) =>
			minimum + Math.floor(next() * (maximum - minimum + 1)),
		shuffle(items) {
			const copy = [...items];
			for (let index = copy.length - 1; index > 0; index -= 1) {
				const swap = Math.floor(next() * (index + 1));
				[copy[index], copy[swap]] = [copy[swap], copy[index]];
			}
			return copy;
		},
	};
}

/** Round to a fixed decimal grid so serialized workloads are platform independent. */
export function quantize(value, decimals = 6) {
	if (!Number.isFinite(value)) throw new Error(`cannot quantize ${value}`);
	const scale = 10 ** decimals;
	const rounded = Math.round(value * scale) / scale;
	return Object.is(rounded, -0) ? 0 : rounded;
}

/** JSON with recursively sorted object keys; the stable input to every digest. */
export function canonicalJson(value) {
	return JSON.stringify(sortKeys(value));
}

function sortKeys(value) {
	if (Array.isArray(value)) return value.map(sortKeys);
	if (value && typeof value === "object")
		return Object.fromEntries(
			Object.keys(value)
				.sort()
				.filter((key) => value[key] !== undefined)
				.map((key) => [key, sortKeys(value[key])]),
		);
	return value;
}

export function sha256(value) {
	return createHash("sha256")
		.update(typeof value === "string" ? value : canonicalJson(value))
		.digest("hex");
}

/**
 * Default evidence directory for one exact input set. `workloadId` names the seed/version/request;
 * the manifest digest separates different patches or content that share it, so a report written
 * next to its manifest always resolves to the inputs it was measured with. Identical
 * regeneration reuses the same directory.
 */
export function semanticWorkloadDirectory(root, manifest) {
	if (!manifest?.workloadId || !/^[0-9a-f]{64}$/u.test(manifest?.manifestSha256 ?? ""))
		throw new Error("a semantic workload directory needs a workloadId and manifestSha256");
	return path.join(root, "semantic-workloads", manifest.workloadId, manifest.manifestSha256.slice(0, 16));
}

export function fileSha256(file) {
	return createHash("sha256").update(fs.readFileSync(file)).digest("hex");
}

// ---------------------------------------------------------------------------------------------
// Strict JSON Schema subset validator

const ANNOTATIONS = new Set([
	"$schema",
	"$defs",
	"title",
	"description",
	"default",
	"examples",
]);
const INTEGER_FORMATS = Object.freeze({
	uint8: [0, 255],
	uint16: [0, 65_535],
	uint32: [0, 4_294_967_295],
	uint64: [0, Number.MAX_SAFE_INTEGER],
	int32: [-2_147_483_648, 2_147_483_647],
	int64: [Number.MIN_SAFE_INTEGER, Number.MAX_SAFE_INTEGER],
});
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/iu;

const schemaCache = new Map();

export function loadWireSchema(relativeFile, root = repositoryRoot) {
	const file = path.resolve(root, relativeFile);
	if (!schemaCache.has(file))
		schemaCache.set(file, JSON.parse(fs.readFileSync(file, "utf8")));
	return schemaCache.get(file);
}

/**
 * Validate `value` against `#/$defs/<definition>` of a generated wire schema. Unknown schema
 * keywords throw instead of being ignored, so a schema change can never silently weaken this.
 */
export function validateWireDefinition(contract, value, root = repositoryRoot) {
	const schema = loadWireSchema(contract.file, root);
	const definition = schema.$defs?.[contract.definition];
	if (!definition)
		throw new Error(`${contract.file} has no $defs/${contract.definition}`);
	const errors = [];
	validateNode(schema, definition, value, `$`, errors);
	return errors;
}

function resolveRef(rootSchema, ref) {
	if (!ref.startsWith("#/")) throw new Error(`unsupported external $ref ${ref}`);
	return ref
		.slice(2)
		.split("/")
		.reduce((node, key) => {
			const next = node?.[key.replaceAll("~1", "/").replaceAll("~0", "~")];
			if (next === undefined) throw new Error(`unresolved $ref ${ref}`);
			return next;
		}, rootSchema);
}

function typeMatches(type, value) {
	switch (type) {
		case "object":
			return value !== null && typeof value === "object" && !Array.isArray(value);
		case "array":
			return Array.isArray(value);
		case "string":
			return typeof value === "string";
		case "integer":
			return Number.isInteger(value);
		case "number":
			return typeof value === "number" && Number.isFinite(value);
		case "boolean":
			return typeof value === "boolean";
		case "null":
			return value === null;
		default:
			throw new Error(`unsupported JSON Schema type ${type}`);
	}
}

function validateNode(rootSchema, schema, value, at, errors) {
	if (schema === true) return;
	if (schema === false) {
		errors.push(`${at}: no value is allowed`);
		return;
	}
	for (const keyword of Object.keys(schema)) {
		if (ANNOTATIONS.has(keyword)) continue;
		switch (keyword) {
			case "$ref":
				validateNode(rootSchema, resolveRef(rootSchema, schema.$ref), value, at, errors);
				break;
			case "type": {
				const types = Array.isArray(schema.type) ? schema.type : [schema.type];
				if (!types.some((type) => typeMatches(type, value)))
					errors.push(`${at}: expected ${types.join("|")}`);
				break;
			}
			case "const":
				if (canonicalJson(value) !== canonicalJson(schema.const))
					errors.push(`${at}: expected constant ${JSON.stringify(schema.const)}`);
				break;
			case "enum":
				if (!schema.enum.some((option) => canonicalJson(option) === canonicalJson(value)))
					errors.push(`${at}: ${JSON.stringify(value)} is not one of ${schema.enum.join(", ")}`);
				break;
			case "format":
				validateFormat(schema.format, value, at, errors);
				break;
			case "minimum":
				if (typeof value === "number" && value < schema.minimum)
					errors.push(`${at}: ${value} is below ${schema.minimum}`);
				break;
			case "maximum":
				if (typeof value === "number" && value > schema.maximum)
					errors.push(`${at}: ${value} is above ${schema.maximum}`);
				break;
			case "minLength":
			case "maxLength":
				if (typeof value === "string") {
					const length = [...value].length;
					if (keyword === "minLength" ? length < schema.minLength : length > schema.maxLength)
						errors.push(`${at}: string length ${length} violates ${keyword}`);
				}
				break;
			case "minItems":
			case "maxItems":
				if (Array.isArray(value)) {
					if (keyword === "minItems" ? value.length < schema.minItems : value.length > schema.maxItems)
						errors.push(`${at}: array length ${value.length} violates ${keyword}`);
				}
				break;
			case "items":
				if (Array.isArray(value)) {
					if (Array.isArray(schema.items))
						throw new Error("tuple-form items is not supported");
					value.forEach((item, index) =>
						validateNode(rootSchema, schema.items, item, `${at}[${index}]`, errors),
					);
				}
				break;
			case "required":
				if (typeMatches("object", value))
					for (const key of schema.required)
						if (!(key in value)) errors.push(`${at}: missing required ${key}`);
				break;
			case "properties":
				if (typeMatches("object", value))
					for (const [key, child] of Object.entries(schema.properties))
						if (key in value)
							validateNode(rootSchema, child, value[key], `${at}.${key}`, errors);
				break;
			case "additionalProperties":
				if (typeMatches("object", value)) {
					const known = new Set(Object.keys(schema.properties ?? {}));
					for (const key of Object.keys(value))
						if (!known.has(key))
							validateNode(rootSchema, schema.additionalProperties, value[key], `${at}.${key}`, errors);
				}
				break;
			case "anyOf":
			case "oneOf": {
				const matches = schema[keyword].filter((option) => {
					const optionErrors = [];
					validateNode(rootSchema, option, value, at, optionErrors);
					return optionErrors.length === 0;
				}).length;
				if (keyword === "anyOf" ? matches === 0 : matches !== 1)
					errors.push(`${at}: ${matches} ${keyword} alternatives matched`);
				break;
			}
			default:
				throw new Error(`unsupported JSON Schema keyword ${keyword} at ${at}`);
		}
	}
}

function validateFormat(format, value, at, errors) {
	if (format === "uuid") {
		if (typeof value === "string" && !UUID.test(value))
			errors.push(`${at}: ${value} is not a UUID`);
	} else if (format in INTEGER_FORMATS) {
		const [minimum, maximum] = INTEGER_FORMATS[format];
		if (typeof value === "number" && (!Number.isInteger(value) || value < minimum || value > maximum))
			errors.push(`${at}: ${value} is not ${format}`);
	} else if (format === "float" || format === "double") {
		if (typeof value === "number" && !Number.isFinite(value))
			errors.push(`${at}: ${value} is not a finite ${format}`);
	} else {
		throw new Error(`unsupported JSON Schema format ${format} at ${at}`);
	}
}

// ---------------------------------------------------------------------------------------------
// Rust storage rules that the JSON Schema cannot express

const REPRESENTATION_OWNER = Object.freeze({
	angles: "position",
	target: "position",
	semantic_color: "color",
	direct_color: "color",
	focus: "focus",
	zoom: "zoom",
});
const COLOR_COMPONENT_DOMAINS = Object.freeze({
	red: [0, 1],
	green: [0, 1],
	blue: [0, 1],
	amber: [0, 1],
	saturation: [0, 1],
	white_blend: [0, 1],
	uv: [0, 1],
	hue: [0, 360],
	temperature: null,
	duv: null,
	relative_output: [0, Number.MAX_VALUE],
});

/** Domain of a component scalar, mirroring `ProgrammingComponent::descriptor`. */
export function componentDomain(component) {
	switch (component.kind) {
		case "pan":
		case "tilt":
		case "target_x":
		case "target_y":
		case "target_z":
			return [Number.NEGATIVE_INFINITY, Number.POSITIVE_INFINITY];
		case "focus":
			return [0, 1];
		case "zoom":
			return [0, 180];
		case "color":
			return COLOR_COMPONENT_DOMAINS[component.component] ?? null;
		default:
			return null;
	}
}

function componentOwner(component) {
	if (["pan", "tilt", "target_x", "target_y", "target_z", "target_reference"].includes(component.kind))
		return "position";
	if (["color", "color_wheel", "native_color"].includes(component.kind)) return "color";
	return component.kind;
}

function addressErrors(address) {
	const { representation, component } = address;
	const errors = [];
	if (!component) {
		if (
			representation.kind === "semantic_color" &&
			representation.basis !== "whole"
		)
			errors.push("whole Color lanes require the Whole basis");
		return errors;
	}
	if (componentOwner(component) !== REPRESENTATION_OWNER[representation.kind])
		errors.push("Dynamic component belongs to a different owner");
	const colorComponent = component.component;
	const compatible =
		(representation.kind === "angles" && ["pan", "tilt"].includes(component.kind)) ||
		(representation.kind === "target" &&
			representation.reference != null &&
			["target_x", "target_y", "target_z"].includes(component.kind)) ||
		(representation.kind === "semantic_color" &&
			component.kind === "color" &&
			(["red", "green", "blue", "amber"].includes(colorComponent)
				? representation.basis === "recipe"
				: ["hue", "saturation"].includes(colorComponent)
					? representation.basis === "hue_saturation"
					: true)) ||
		(representation.kind === "focus" && component.kind === "focus") ||
		(representation.kind === "zoom" && component.kind === "zoom");
	if (!compatible)
		errors.push("Dynamic component is incompatible with its declared representation");
	if (
		representation.kind === "target" &&
		representation.reference?.kind === "point" &&
		/^0{8}-0{4}-0{4}-0{4}-0{12}$/u.test(representation.reference.point_id)
	)
		errors.push("Dynamic target requires a stable Point UUID");
	return errors;
}

function sourceErrors(source, address) {
	if (source.kind === "current") return [];
	if (source.kind !== "value") return [`unsupported source ${source.kind} in synthetic workload`];
	const value = source.value;
	if (!address.component) return value.kind === "family" ? [] : ["whole lanes need family values"];
	if (value.kind !== "scalar") return ["component lanes need scalar values"];
	const domain = componentDomain(address.component);
	if (!domain) return ["component has no Dynamic scalar domain"];
	const [minimum, maximum] = domain;
	const inside = Number.isFinite(value.value) && value.value >= minimum && value.value <= maximum;
	return inside ? [] : ["Dynamic scalar is outside its declared component domain"];
}

/** Programming lane and definition rules mirrored from the dynamics domain crate. */
export function semanticDefinitionErrors(definition) {
	const errors = [];
	const angles = { pan: 0, tilt: 0, whole: 0 };
	for (const lane of definition.lanes) {
		const body = lane.programming;
		if (!body) {
			errors.push(`lane ${lane.id}: semantic workloads use programming lanes only`);
			continue;
		}
		const { address, configuration } = body;
		errors.push(...addressErrors(address).map((error) => `lane ${lane.id}: ${error}`));
		const config = configuration.configuration;
		const sources =
			configuration.mode === "keyframes"
				? config.points.map((point) => point.source)
				: configuration.mode === "max_min"
					? [config.minimum, config.maximum]
					: configuration.mode === "middle_amplitude"
						? [config.middle]
						: [];
		for (const source of sources)
			errors.push(...sourceErrors(source, address).map((error) => `lane ${lane.id}: ${error}`));
		if (configuration.mode === "keyframes") {
			const positions = config.points.map((point) => point.position);
			if (
				positions.length < 2 ||
				positions[0] !== 0 ||
				positions.some((position, index) => position < 0 || position >= 1 || (index > 0 && position <= positions[index - 1]))
			)
				errors.push(`lane ${lane.id}: keyframe positions must be strictly increasing in [0, 1)`);
			if (!address.component && config.size !== 1)
				errors.push(`lane ${lane.id}: whole-family keyframes require unit lane size`);
		} else if (!address.component) {
			errors.push(`lane ${lane.id}: numeric configuration requires a component`);
		}
		if (configuration.mode === "middle_amplitude") {
			const amplitude = config.amplitude;
			if (amplitude.kind !== "scalar" || !(amplitude.value >= 0))
				errors.push(`lane ${lane.id}: amplitude must be a nonnegative component delta`);
		}
		if (address.representation.kind === "angles") {
			const axis = address.component?.kind ?? "whole";
			angles[axis] += 1;
		}
	}
	const pair = `${angles.pan}${angles.tilt}${angles.whole}`;
	if (!["000", "110", "001"].includes(pair))
		errors.push("an Angle Dynamic needs one complete Pan/Tilt pair or one whole Angles lane");
	const ids = definition.lanes.map((lane) => lane.id);
	if (new Set(ids).size !== ids.length) errors.push("lane IDs must be unique");
	return errors;
}

/** Full validation: generated JSON Schema plus mirrored storage rules. */
export function validateSemanticDefinition(definition, root = repositoryRoot) {
	return [
		...validateWireDefinition(SEMANTIC_CONTRACT_SCHEMAS.dynamicDefinition, definition, root),
		...semanticDefinitionErrors(definition),
	];
}

/** Digests of the contract files a workload was validated against. */
export function contractIdentity(root = repositoryRoot) {
	const files = [
		...new Set([
			...Object.values(SEMANTIC_CONTRACT_SCHEMAS).map((contract) => contract.file),
			GENERATED_WIRE_TYPES,
		]),
	].sort();
	return Object.fromEntries(
		files.map((file) => {
			const absolute = path.resolve(root, file);
			return [
				file,
				fs.existsSync(absolute)
					? { sha256: fileSha256(absolute) }
					: { status: "unavailable", reason: "contract file is missing" },
			];
		}),
	);
}

export { repositoryRoot as semanticContractRepositoryRoot };
