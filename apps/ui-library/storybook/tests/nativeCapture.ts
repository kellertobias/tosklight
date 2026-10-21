import type { Page } from "@playwright/test";

/** Re-encode a real capture at its original size; never redraw or alter native UI content. */
export async function nativeCapturePng(
	page: Page,
	source: Buffer,
): Promise<Buffer> {
	if (source.subarray(1, 4).toString("ascii") === "PNG") return source;
	if (source[0] !== 0xff || source[1] !== 0xd8)
		throw new Error("Native screenshot source must be PNG or JPEG");
	const png = await page.evaluate(async (encoded) => {
		const original = new Image();
		original.src = `data:image/jpeg;base64,${encoded}`;
		await original.decode();
		const canvas = document.createElement("canvas");
		canvas.width = original.naturalWidth;
		canvas.height = original.naturalHeight;
		const context = canvas.getContext("2d");
		if (!context)
			throw new Error("Cannot normalize native screenshot encoding");
		context.drawImage(original, 0, 0);
		return canvas.toDataURL("image/png").split(",")[1];
	}, source.toString("base64"));
	return Buffer.from(png, "base64");
}
