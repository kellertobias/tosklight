import { decodeFamilyEncoderPagesSnapshot, familyEncoderPagesPath } from "../familyEncoderPagesWire";
import type { FamilyEncoderPagesSnapshot, NativeColorPagesSnapshot } from "../generated/light-wire";
import {
	decodeNativeColorPagesSnapshot,
	type NativeColorReferenceChoice,
	nativeColorPagesPath,
} from "../nativeColorPagesWire";
import type { ClientTransport } from "./transport";

/**
 * Semantic family encoder pages and displayed-source readouts (TL-549/550/551 UI foundation).
 * Both are authenticated reads; the desk sends the `X-Tosk-Show` guard (api-rules §6).
 */
export class FamilyEncoderApiClient {
	constructor(private readonly transport: ClientTransport) {}

	async pages(
		fixtureIds: readonly string[],
		showId?: string | null,
	): Promise<FamilyEncoderPagesSnapshot> {
		const value = await this.transport.request<unknown>(
			familyEncoderPagesPath(fixtureIds),
			showId ? { headers: { "X-Tosk-Show": showId } } : {},
		);
		return decodeFamilyEncoderPagesSnapshot(value);
	}

	/** TL-554: Direct Color pages 3/4, overflow and reference head. An inert read. */
	async nativeColorPages(
		fixtureIds: readonly string[],
		showId?: string | null,
		reference?: NativeColorReferenceChoice | null,
	): Promise<NativeColorPagesSnapshot> {
		const value = await this.transport.request<unknown>(
			nativeColorPagesPath(fixtureIds, reference),
			showId ? { headers: { "X-Tosk-Show": showId } } : {},
		);
		return decodeNativeColorPagesSnapshot(value);
	}

	/** The `ReadoutRequest` that `DisplayedSourceReadouts` uses. */
	readonly readoutRequest = (
		path: string,
		init: { headers?: Record<string, string> },
	) => this.transport.request<unknown>(path, init);
}
