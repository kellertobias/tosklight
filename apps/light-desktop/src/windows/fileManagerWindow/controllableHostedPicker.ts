import type {
	FileManagerPickerRequest,
	HostedFileManagerPickerResult,
} from "./hostedPickerContract";
import type { FileManagerTarget } from "./types";

export const HOSTED_PICKER_TEST_CONTROL = "__lightHostedPickerTestControl";

export interface ControllableHostedPickerSelection {
	rootId: string;
	name: string;
	path: string;
	kind: "file" | "folder";
}

export interface ControllableSystemFile {
	name: string;
	type: string;
	size: number;
	lastModified: number;
}

export type ControllableHostedPickerOutcome =
	| {
			status: "selected";
			selections: ControllableHostedPickerSelection[];
	  }
	| {
			status: "system_selected";
			target: FileManagerTarget;
			files: ControllableSystemFile[];
			directoryName?: string;
	  }
	| { status: "cancelled" };

export interface ControllableHostedPickerOperation {
	outcome: Promise<ControllableHostedPickerOutcome>;
	cancel(): void;
}

export interface ControllableHostedPickerPort {
	attach(
		handler: (request: unknown) => ControllableHostedPickerOperation,
	): unknown;
	request(request: unknown): Promise<unknown>;
	dispose(): void;
}

export type ControllableHostedPickerWindow = Window & {
	[HOSTED_PICKER_TEST_CONTROL]?: unknown;
};

export function attachControllableHostedPicker(
	open: (
		request: FileManagerPickerRequest,
	) => ControllableHostedPickerOperation,
	runtime: ControllableHostedPickerWindow = window,
): () => void {
	const port = injectedHostedPickerPort(runtime);
	if (!port) return () => undefined;
	const detach = port.attach((request) =>
		open(decodeHostedPickerRequest(request)),
	);
	if (typeof detach !== "function")
		throw new Error("Invalid controllable hosted-picker attachment");
	return detach as () => void;
}

export function decodeHostedPickerRequest(
	value: unknown,
): FileManagerPickerRequest {
	const request = record(value, "hosted-picker request");
	exactKeys(request, REQUEST_KEYS, "hosted-picker request");
	return {
		...optionalString(request, "purpose"),
		...optionalTarget(request),
		...optionalBoolean(request, "multiple"),
		...optionalStringArray(request, "allowedExtensions"),
		...optionalString(request, "initialRootId"),
		...optionalString(request, "initialDirectory"),
		...optionalString(request, "selectLabel"),
		...optionalString(request, "cancelLabel"),
		...optionalBoolean(request, "hideCancel"),
	};
}

export function decodeHostedPickerOutcome(
	value: unknown,
): ControllableHostedPickerOutcome {
	const outcome = record(value, "hosted-picker outcome");
	if (outcome.status === "cancelled") {
		exactKeys(outcome, ["status"], "cancelled hosted-picker outcome");
		return { status: "cancelled" };
	}
	if (outcome.status === "selected") return decodeSelections(outcome);
	if (outcome.status === "system_selected")
		return decodeSystemSelection(outcome);
	throw new Error("Invalid hosted-picker outcome status");
}

export function controllableHostedPickerOutcome(
	result: HostedFileManagerPickerResult,
): ControllableHostedPickerOutcome {
	if (result === null) return { status: "cancelled" };
	if (Array.isArray(result)) {
		return {
			status: "selected",
			selections: result.map(({ rootId, entry }) => ({
				rootId,
				name: entry.name,
				path: entry.path,
				kind: entry.kind,
			})),
		};
	}
	return {
		status: "system_selected",
		target: result.target,
		files: result.files.map(({ name, type, size, lastModified }) => ({
			name,
			type,
			size,
			lastModified,
		})),
		...(result.directoryName ? { directoryName: result.directoryName } : {}),
	};
}

const REQUEST_KEYS = [
	"purpose",
	"target",
	"multiple",
	"allowedExtensions",
	"initialRootId",
	"initialDirectory",
	"selectLabel",
	"cancelLabel",
	"hideCancel",
] as const;

function injectedHostedPickerPort(
	runtime: ControllableHostedPickerWindow,
): ControllableHostedPickerPort | null {
	const value = runtime[HOSTED_PICKER_TEST_CONTROL];
	if (value === undefined) return null;
	const port = record(value, "controllable hosted-picker port");
	exactKeys(
		port,
		["attach", "request", "dispose"],
		"controllable hosted-picker port",
	);
	if (
		typeof port.attach !== "function" ||
		typeof port.request !== "function" ||
		typeof port.dispose !== "function"
	)
		throw new Error("Invalid controllable hosted-picker port");
	return port as unknown as ControllableHostedPickerPort;
}

function decodeSelections(
	outcome: Record<string, unknown>,
): ControllableHostedPickerOutcome {
	exactKeys(
		outcome,
		["status", "selections"],
		"selected hosted-picker outcome",
	);
	if (!Array.isArray(outcome.selections))
		throw new Error("Invalid hosted-picker selections");
	return {
		status: "selected",
		selections: outcome.selections.map((value) => {
			const selection = record(value, "hosted-picker selection");
			exactKeys(
				selection,
				["rootId", "name", "path", "kind"],
				"hosted-picker selection",
			);
			return {
				rootId: stringValue(selection.rootId, "selection root"),
				name: stringValue(selection.name, "selection name"),
				path: stringValue(selection.path, "selection path"),
				kind: fileKind(selection.kind),
			};
		}),
	};
}

function decodeSystemSelection(
	outcome: Record<string, unknown>,
): ControllableHostedPickerOutcome {
	exactKeys(
		outcome,
		["status", "target", "files", "directoryName"],
		"system hosted-picker outcome",
	);
	if (!Array.isArray(outcome.files))
		throw new Error("Invalid hosted-picker system files");
	return {
		status: "system_selected",
		target: targetValue(outcome.target),
		files: outcome.files.map(decodeSystemFile),
		...optionalString(outcome, "directoryName"),
	};
}

function decodeSystemFile(value: unknown): ControllableSystemFile {
	const file = record(value, "hosted-picker system file");
	exactKeys(
		file,
		["name", "type", "size", "lastModified"],
		"hosted-picker system file",
	);
	return {
		name: stringValue(file.name, "system file name"),
		type: stringValue(file.type, "system file type"),
		size: finiteNumber(file.size, "system file size"),
		lastModified: finiteNumber(file.lastModified, "system file modified time"),
	};
}

function optionalTarget(value: Record<string, unknown>) {
	return value.target === undefined
		? {}
		: { target: targetValue(value.target) };
}

function targetValue(value: unknown): FileManagerTarget {
	if (value !== "files" && value !== "folders" && value !== "either")
		throw new Error("Invalid hosted-picker target");
	return value;
}

function fileKind(value: unknown): "file" | "folder" {
	if (value !== "file" && value !== "folder")
		throw new Error("Invalid hosted-picker selection kind");
	return value;
}

function optionalString<K extends string>(
	value: Record<string, unknown>,
	key: K,
): Partial<Record<K, string>> {
	return value[key] === undefined
		? {}
		: ({ [key]: stringValue(value[key], key) } as Record<K, string>);
}

function optionalBoolean<K extends string>(
	value: Record<string, unknown>,
	key: K,
): Partial<Record<K, boolean>> {
	if (value[key] === undefined) return {};
	if (typeof value[key] !== "boolean")
		throw new Error(`Invalid hosted-picker ${key}`);
	return { [key]: value[key] } as Record<K, boolean>;
}

function optionalStringArray(value: Record<string, unknown>, key: string) {
	if (value[key] === undefined) return {};
	if (!Array.isArray(value[key]))
		throw new Error(`Invalid hosted-picker ${key}`);
	return {
		[key]: value[key].map((item) => stringValue(item, key)),
	};
}

function exactKeys(
	value: Record<string, unknown>,
	keys: readonly string[],
	label: string,
): void {
	const allowed = new Set(keys);
	if (Object.keys(value).some((key) => !allowed.has(key)))
		throw new Error(`Invalid ${label} fields`);
}

function record(value: unknown, label: string): Record<string, unknown> {
	if (!value || typeof value !== "object" || Array.isArray(value))
		throw new Error(`Invalid ${label}`);
	return value as Record<string, unknown>;
}

function stringValue(value: unknown, label: string): string {
	if (typeof value !== "string") throw new Error(`Invalid ${label}`);
	return value;
}

function finiteNumber(value: unknown, label: string): number {
	if (typeof value !== "number" || !Number.isFinite(value) || value < 0)
		throw new Error(`Invalid ${label}`);
	return value;
}
