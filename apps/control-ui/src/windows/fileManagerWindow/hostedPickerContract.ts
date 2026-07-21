import type {
	FileManagerPickerOptions,
	FileManagerSelection,
	FileManagerTarget,
} from "./types";

export type FileManagerPickerRequest = Omit<
	FileManagerPickerOptions,
	"onSelect" | "onCancel"
>;

export interface SystemFilePickerSelection {
	source: "system";
	target: FileManagerTarget;
	files: File[];
	directoryName?: string;
}

export type HostedFileManagerPickerResult =
	| FileManagerSelection[]
	| SystemFilePickerSelection
	| null;

export interface HostedPickerRequest extends FileManagerPickerRequest {
	onSelect: (selection: FileManagerSelection[]) => void;
	onSystemSelect: (selection: SystemFilePickerSelection) => void;
	onCancel: () => void;
}

export interface HostedPickerOperation {
	request: HostedPickerRequest;
	result: Promise<HostedFileManagerPickerResult>;
}

export function createHostedPickerOperation(
	options: FileManagerPickerRequest,
): HostedPickerOperation {
	let settle: ((result: HostedFileManagerPickerResult) => void) | null = null;
	const result = new Promise<HostedFileManagerPickerResult>((resolve) => {
		settle = resolve;
	});
	const complete = (value: HostedFileManagerPickerResult) => {
		if (!settle) return;
		const resolve = settle;
		settle = null;
		resolve(value);
	};
	return {
		request: {
			...options,
			onSelect: complete,
			onSystemSelect: complete,
			onCancel: () => complete(null),
		},
		result,
	};
}
