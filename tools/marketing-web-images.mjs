import { mkdirSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import sharp from "sharp";

// Source captures stay full resolution; every website image, including enlarged views,
// uses a bounded derivative. Mobile browsers can choose a smaller transfer from srcset.
export async function marketingWebImages(entries, sourceDirectory, destination) {
  mkdirSync(destination, { recursive: true });
  const images = new Map();
  for (const { file } of entries) {
    const source = resolve(sourceDirectory, file);
    const name = file.replaceAll("/", "-").replace(/\.png$/u, ".jpg");
    const metadata = await sharp(source).metadata();
    const variants = [];
    for (const width of [...new Set([640, 960, Math.min(1600, metadata.width)].filter(width => width <= metadata.width))].sort((a,b) => a-b)) {
      const base = width === Math.min(1600, metadata.width);
      const filename = base ? name : name.replace(/\.jpg$/u, `-${width}w.jpg`);
      const { data, info } = await sharp(source).flatten({ background: "#10131a" })
        .resize({ width, withoutEnlargement: true })
        .jpeg({ quality: 76, progressive: true, mozjpeg: true }).toBuffer({ resolveWithObject: true });
      writeFileSync(resolve(destination, filename), data);
      variants.push({ file: filename, width: info.width, height: info.height, bytes: data.length });
    }
    const largest = variants.at(-1);
    images.set(file, { ...largest, variants, sourceWidth: metadata.width, sourceHeight: metadata.height });
  }
  // A reused Pages artifact must not retain the previous multi-megabyte PNG originals.
  for (const name of readdirSync(destination)) {
    if (name.endsWith(".png")) rmSync(resolve(destination, name));
  }
  writeFileSync(resolve(destination, "web-images.json"), JSON.stringify(Object.fromEntries(images), null, 2) + "\n");
  return images;
}

export function responsiveScreenshot(image, prefix = "screenshots/") {
  return `src="${prefix}${image.file}" srcset="${image.variants.map(v => `${prefix}${v.file} ${v.width}w`).join(", ")}" sizes="(max-width: 700px) 100vw, (max-width: 1100px) 80vw, 1200px" width="${image.width}" height="${image.height}"`;
}
