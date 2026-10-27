import type { DesktopBridge, StagePaneGeometry } from "../../platform/desktop/types";

type AttachmentBridge = Pick<DesktopBridge, "openStagePane" | "setStagePane" | "setStagePaneVisibility" | "closeStagePane">;
type Serialize = (work: () => Promise<void>) => Promise<void>;

/** One latest geometry/visibility transition, serialized with native attachment ownership. */
export class NativeStageAttachment {
	private disposed = false;
	private opened = false;
	private announced = false;
	private attempted = false;
	private pending = false;
	private version = 0;
	private processed = 0;
	private geometry: StagePaneGeometry | null = null;
	private sentGeometry: string | null = null;
	private visible = false;
	private sentVisibility: boolean | null = null;

	constructor(private readonly bridge: AttachmentBridge, private readonly id: string,
		private readonly live3d: boolean, private readonly serialize: Serialize,
		private readonly attached: () => void, private readonly failed: (error: unknown) => void) {}

	update(geometry: StagePaneGeometry | null, visible: boolean) {
		if (this.disposed) return;
		if (JSON.stringify(geometry) === JSON.stringify(this.geometry) && visible === this.visible) return;
		this.geometry = geometry;
		this.visible = visible && geometry !== null;
		this.version++;
		this.schedule();
	}

	dispose() {
		if (this.disposed) return;
		this.disposed = true;
		void this.serialize(async () => {
			if (this.attempted) await this.bridge.closeStagePane(this.id);
		}).catch(this.failed);
	}

	private schedule() {
		if (this.pending || this.disposed) return;
		this.pending = true;
		void this.serialize(() => this.flush()).catch((error) => {
			this.processed = this.version; // Report errors; never spin on an unchanged failed request.
			if (!this.disposed) this.failed(error);
		}).finally(() => {
			this.pending = false;
			if (!this.disposed && this.processed !== this.version) this.schedule();
		});
	}

	private async flush() {
		while (!this.disposed) {
			const version = this.version;
			if (!this.opened && this.geometry) {
				const geometry = this.geometry;
				this.attempted = true;
				await this.bridge.openStagePane(this.id, this.live3d, geometry);
				this.opened = true;
				this.sentGeometry = JSON.stringify(geometry);
			}
			if (this.disposed) return;
			if (this.opened) {
				// Suspend first. Resizes while hidden cannot revive the picture.
				if (!this.visible) await this.sendVisibility(false);
				const geometry = this.geometry;
				if (geometry && JSON.stringify(geometry) !== this.sentGeometry) {
					await this.bridge.setStagePane(this.id, geometry);
					this.sentGeometry = JSON.stringify(geometry);
				}
				if (!this.disposed) await this.sendVisibility(this.visible);
			}
			this.processed = version;
			if (version === this.version) {
				if (this.opened && !this.announced) { this.announced = true; this.attached(); }
				return;
			}
		}
	}

	private async sendVisibility(visible: boolean) {
		if (this.disposed || this.sentVisibility === visible) return;
		await this.bridge.setStagePaneVisibility(this.id, visible);
		this.sentVisibility = visible;
	}
}
