import { expect, test } from "@playwright/test";
import { nativeCapturePng } from "./nativeCapture";

test("native screenshot normalization preserves decoded JPEG pixels and dimensions", async ({
	page,
}) => {
	const jpeg = await page.evaluate(() => {
		const canvas = document.createElement("canvas");
		canvas.width = 80;
		canvas.height = 60;
		const context = canvas.getContext("2d")!;
		const gradient = context.createLinearGradient(0, 0, 80, 60);
		gradient.addColorStop(0, "#123456");
		gradient.addColorStop(1, "#f0a020");
		context.fillStyle = gradient;
		context.fillRect(0, 0, 80, 60);
		return canvas.toDataURL("image/jpeg").split(",")[1];
	});
	const source = Buffer.from(jpeg, "base64");
	const png = await nativeCapturePng(page, source);
	expect(png.subarray(1, 4).toString("ascii")).toBe("PNG");
	expect(png.readUInt32BE(16)).toBe(80);
	expect(png.readUInt32BE(20)).toBe(60);
	const identical = await page.evaluate(
		async ({ jpeg, png }) => {
			const pixels = async (mime: string, encoded: string) => {
				const image = new Image();
				image.src = `data:${mime};base64,${encoded}`;
				await image.decode();
				const canvas = document.createElement("canvas");
				canvas.width = image.naturalWidth;
				canvas.height = image.naturalHeight;
				const context = canvas.getContext("2d")!;
				context.drawImage(image, 0, 0);
				return context.getImageData(0, 0, canvas.width, canvas.height).data;
			};
			const before = await pixels("image/jpeg", jpeg);
			const after = await pixels("image/png", png);
			return (
				before.length === after.length &&
				before.every((value, index) => value === after[index])
			);
		},
		{ jpeg, png: png.toString("base64") },
	);
	expect(identical).toBe(true);
	expect(await nativeCapturePng(page, png)).toBe(png);
	await expect(
		nativeCapturePng(page, Buffer.from("invalid image")),
	).rejects.toThrow("Native screenshot source must be PNG or JPEG");
});
