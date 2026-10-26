import { createHash } from "node:crypto";
import { readFile, realpath, stat } from "node:fs/promises";
import path from "node:path";
import { inflateSync } from "node:zlib";

export function captureRequested(env, baseline, port, profile) {
    const value = env.LIGHT_STAGE_PACKAGED_BENCH_CAPTURE;
    if (value === undefined) return false;
    if (value !== "live-ultra" || port !== 5160 || baseline?.layout !== "dual" || baseline.phases ||
        env.LIGHT_STAGE_PACKAGED_BENCH_DISPLAY !== "internal" || env.LIGHT_STAGE_PACKAGED_BENCH_STAGE === "0" ||
        !["canonical-demo", "stage-500"].includes(profile))
        throw new Error("Capture exercise requires live-ultra, private port 5160, fixed dual Stage layout, explicit internal display, canonical-demo/stage-500 and no baseline phases");
    return true;
}

const crcTable = Uint32Array.from({ length: 256 }, (_, index) => {
    let crc = index;
    for (let bit = 0; bit < 8; bit++) crc = crc & 1 ? 0xedb88320 ^ (crc >>> 1) : crc >>> 1;
    return crc >>> 0;
});
function pngCrc(bytes) {
    let crc = 0xffffffff;
    for (const byte of bytes) crc = crcTable[(crc ^ byte) & 255] ^ (crc >>> 8);
    return (crc ^ 0xffffffff) >>> 0;
}

/** Decode the native writer's bounded non-interlaced RGBA8 PNG payload; never spawn a viewer. */
export function inspectNativePng(bytes) {
    if (bytes.length > 64 * 1024 * 1024 || !bytes.subarray(0, 8).equals(Buffer.from([137,80,78,71,13,10,26,10])))
        throw new Error("Saved image is not a bounded PNG");
    let offset = 8, width, height, ended = false;
    const compressed = [];
    while (offset + 12 <= bytes.length) {
        const length = bytes.readUInt32BE(offset), type = bytes.toString("ascii", offset + 4, offset + 8);
        if (offset + length + 12 > bytes.length) throw new Error("Truncated PNG chunk");
        if (pngCrc(bytes.subarray(offset + 4, offset + 8 + length)) !== bytes.readUInt32BE(offset + 8 + length))
            throw new Error("PNG chunk CRC mismatch");
        const data = bytes.subarray(offset + 8, offset + 8 + length);
        if (type === "IHDR") {
            if (offset !== 8 || length !== 13 || data[8] !== 8 || data[9] !== 6 || data[10] || data[11] || data[12])
                throw new Error("Expected native non-interlaced RGBA8 PNG");
            width = data.readUInt32BE(0); height = data.readUInt32BE(4);
        } else if (type === "IDAT") compressed.push(data);
        else if (type === "IEND") { ended = length === 0 && offset + 12 === bytes.length; break; }
        offset += length + 12;
    }
    if (!ended || width !== 3840 || height !== 2160 || !compressed.length) throw new Error("PNG is incomplete or not 3840×2160");
    const stride = width * 4, raw = inflateSync(Buffer.concat(compressed), { maxOutputLength: (stride + 1) * height });
    if (raw.length !== (stride + 1) * height) throw new Error("Invalid PNG scanline size");
    const previous = Buffer.alloc(stride), current = Buffer.alloc(stride);
    for (let y = 0; y < height; y++) {
        const row = y * (stride + 1), filter = raw[row];
        if (filter > 4) throw new Error("Invalid PNG scanline filter");
        for (let x = 0; x < stride; x++) {
            const a = x < 4 ? 0 : current[x - 4], b = previous[x], c = x < 4 ? 0 : previous[x - 4];
            const estimate = a + b - c, da = Math.abs(estimate - a), db = Math.abs(estimate - b), dc = Math.abs(estimate - c);
            const predictor = filter === 0 ? 0 : filter === 1 ? a : filter === 2 ? b : filter === 3 ? (a + b) >> 1 : da <= db && da <= dc ? a : db <= dc ? b : c;
            current[x] = (raw[row + x + 1] + predictor) & 255;
        }
        current.copy(previous);
    }
    return { width, height, bytes: bytes.length, sha256: createHash("sha256").update(bytes).digest("hex"), decoded: true };
}

export async function evaluateCapture(record, outputDirectory, metadata) {
    const failures = [...(record?.failures ?? [])];
    let image = null;
    try {
        const status = record?.status;
        if (record?.schemaVersion !== 2 || !record.passed || record.ready?.kind !== "stage-ready" || !record.ui?.buttonClicked || !record.ui?.savedVisible ||
            status?.engine !== "ultra" || !status.phase?.saved?.path || status.followPreload ||
            status.requestId !== record.request?.requestId || status.paneId !== record.request?.paneId)
            throw new Error("Capture UI/status evidence is incomplete");
        const destination = await realpath(status.phase.saved.path), directory = await realpath(outputDirectory);
        if (path.dirname(destination) !== directory || !path.basename(destination).endsWith(`-${status.jobId}.png`))
            throw new Error("Saved PNG does not belong to this isolated job");
        if ((await stat(destination)).size > 64 * 1024 * 1024) throw new Error("Saved PNG exceeds bounded inspection size");
        image = { path: destination, ...inspectNativePng(await readFile(destination)), jobId: status.jobId };
    } catch (error) { failures.push(String(error)); }
    return { schemaVersion: 2, kind: "packaged-stage-capture-verification", ...metadata,
        capture: record, image, failures, captureVerified: failures.length === 0,
        acceptanceGateEnforced: false, baselineMeasurementReady: false,
        boundary: "DOM-to-native saved image integration only; not physical input, fidelity, 60 FPS or human acceptance" };
}
