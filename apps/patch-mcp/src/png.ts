/** A minimal PNG writer and circle rasterizer for snapshots, so no image library is needed. */

import { deflateSync } from "node:zlib";

const CRC_TABLE = Array.from({ length: 256 }, (_, index) => {
	let value = index;
	for (let bit = 0; bit < 8; bit++) value = value & 1 ? 0xedb88320 ^ (value >>> 1) : value >>> 1;
	return value >>> 0;
});

function crc32(bytes: Uint8Array) {
	let crc = 0xffffffff;
	for (const byte of bytes) crc = CRC_TABLE[(crc ^ byte) & 0xff] ^ (crc >>> 8);
	return (crc ^ 0xffffffff) >>> 0;
}

function chunk(type: string, data: Uint8Array) {
	const out = new Uint8Array(12 + data.length);
	const view = new DataView(out.buffer);
	view.setUint32(0, data.length);
	for (let index = 0; index < 4; index++) out[4 + index] = type.charCodeAt(index);
	out.set(data, 8);
	view.setUint32(8 + data.length, crc32(out.subarray(4, 8 + data.length)));
	return out;
}

export class Raster {
	readonly pixels: Uint8Array;
	constructor(
		readonly width: number,
		readonly height: number,
		background: [number, number, number],
	) {
		this.pixels = new Uint8Array(width * height * 3);
		for (let index = 0; index < width * height; index++) this.pixels.set(background, index * 3);
	}

	/** A filled disc blended over what is there, with a one-pixel outline. */
	disc(cx: number, cy: number, radius: number, color: [number, number, number], alpha: number, outline: [number, number, number]) {
		for (let y = Math.floor(cy - radius - 1); y <= Math.ceil(cy + radius + 1); y++) {
			if (y < 0 || y >= this.height) continue;
			for (let x = Math.floor(cx - radius - 1); x <= Math.ceil(cx + radius + 1); x++) {
				if (x < 0 || x >= this.width) continue;
				const distance = Math.hypot(x - cx, y - cy);
				const at = (y * this.width + x) * 3;
				if (distance <= radius - 1) {
					for (let channel = 0; channel < 3; channel++)
						this.pixels[at + channel] = Math.round(this.pixels[at + channel] * (1 - alpha) + color[channel] * alpha);
				} else if (distance <= radius + 0.5) this.pixels.set(outline, at);
			}
		}
	}

	png(): Uint8Array {
		const rows = new Uint8Array(this.height * (this.width * 3 + 1));
		for (let y = 0; y < this.height; y++) {
			rows[y * (this.width * 3 + 1)] = 0;
			rows.set(this.pixels.subarray(y * this.width * 3, (y + 1) * this.width * 3), y * (this.width * 3 + 1) + 1);
		}
		const header = new Uint8Array(13);
		const view = new DataView(header.buffer);
		view.setUint32(0, this.width);
		view.setUint32(4, this.height);
		header.set([8, 2, 0, 0, 0], 8);
		const parts = [
			new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10]),
			chunk("IHDR", header),
			chunk("IDAT", deflateSync(rows)),
			chunk("IEND", new Uint8Array()),
		];
		const out = new Uint8Array(parts.reduce((total, part) => total + part.length, 0));
		let offset = 0;
		for (const part of parts) {
			out.set(part, offset);
			offset += part.length;
		}
		return out;
	}
}

export function base64(bytes: Uint8Array) {
	let binary = "";
	for (let index = 0; index < bytes.length; index += 0x8000) binary += String.fromCharCode(...bytes.subarray(index, index + 0x8000));
	return btoa(binary);
}
