import type {
	FileDirectory,
	FileInputAction,
	FileInputContext,
	FileMetadata,
	FileNativeNote,
	FileOperationInput,
	FileOperationResult,
	FileRoot,
	TextDocument,
} from "../types";
import type { ClientTransport } from "./transport";
import { jsonRequest } from "./transport";

export class FileApiClient {
	constructor(private readonly transport: ClientTransport) {}

	fileRoots(): Promise<FileRoot[]> {
		return this.transport.request("/api/v1/files/roots");
	}

	fileEntries(root: string, path = "", hidden = false): Promise<FileDirectory> {
		const query = `path=${encodeURIComponent(path)}&hidden=${hidden}`;
		return this.transport.request(`${filePath(root, "entries")}?${query}`);
	}

	fileMetadata(root: string, path: string): Promise<FileMetadata> {
		return this.transport.request(
			`${filePath(root, "metadata")}${pathQuery(path)}`,
		);
	}

	readFileNote(root: string, path: string): Promise<FileNativeNote> {
		return this.transport.request(
			`${filePath(root, "notes")}${pathQuery(path)}`,
		);
	}

	saveFileNote(
		root: string,
		path: string,
		note: string,
	): Promise<FileNativeNote> {
		return this.transport.request(
			filePath(root, "notes"),
			jsonRequest("PUT", { path, note }),
		);
	}

	readTextFile(root: string, path: string): Promise<TextDocument> {
		return this.transport.request(
			`${filePath(root, "text")}${pathQuery(path)}`,
		);
	}

	saveTextFile(
		root: string,
		path: string,
		text: string,
		revision: string | null,
	): Promise<TextDocument> {
		return this.transport.request(
			filePath(root, "text"),
			jsonRequest("PUT", { path, text, revision }),
		);
	}

	fileOperation(
		root: string,
		input: FileOperationInput,
	): Promise<FileOperationResult> {
		return this.transport.request(
			filePath(root, "operations"),
			jsonRequest("POST", { sources: [], ...input }),
		);
	}

	fileContent(root: string, path: string): Promise<Blob> {
		return this.transport.blob(
			`${filePath(root, "content")}${pathQuery(path)}`,
		);
	}

	async fileStreamUrl(root: string, path: string): Promise<string> {
		const response = await this.transport.request<{ ticket: string }>(
			filePath(root, "stream-ticket"),
			jsonRequest("POST", { path }),
		);
		const query = `${pathQuery(path)}&ticket=${encodeURIComponent(response.ticket)}`;
		return this.transport.absoluteUrl(`${filePath(root, "content")}${query}`);
	}

	fileThumbnail(root: string, path: string, maxSize = 256): Promise<Blob> {
		const query = `${pathQuery(path)}&max_size=${maxSize}`;
		return this.transport.blob(`${filePath(root, "thumbnail")}${query}`);
	}

	claimFileInput(
		instanceId: string,
		action: FileInputAction,
		origin: "pending" | "toolbar",
	): Promise<FileInputContext> {
		return this.transport.request(
			"/api/v1/files/input-context",
			jsonRequest("POST", { instance_id: instanceId, action, origin }),
		);
	}

	releaseFileInput(instanceId: string): Promise<void> {
		const query = `?instance_id=${encodeURIComponent(instanceId)}`;
		return this.transport.request(`/api/v1/files/input-context${query}`, {
			method: "DELETE",
		});
	}
}

function filePath(root: string, suffix: string): string {
	return `/api/v1/files/${encodeURIComponent(root)}/${suffix}`;
}

function pathQuery(path: string): string {
	return `?path=${encodeURIComponent(path)}`;
}
