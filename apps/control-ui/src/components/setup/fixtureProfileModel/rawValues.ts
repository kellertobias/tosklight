import type { ChannelResolution } from "../../../api/types";

const RESOLUTION_BYTES: Record<ChannelResolution, number> = {
	u8: 1,
	u16: 2,
	u24: 3,
	u32: 4,
};

const RESOLUTION_MAXIMUMS: Record<ChannelResolution, number> = {
	u8: 0xff,
	u16: 0xffff,
	u24: 0xffffff,
	u32: 0xffffffff,
};

export function resolutionBytes(resolution: ChannelResolution) {
	return RESOLUTION_BYTES[resolution];
}

export function maxRaw(resolution: ChannelResolution) {
	return RESOLUTION_MAXIMUMS[resolution];
}
