export type Rgb = [number, number, number];

/** Buckets this far apart (RGB distance) count as different colours. */
const DISTINCT = 48;

/**
 * The cover's main colours, most prominent first: `rgba` is a small downsample (24×24 is
 * plenty). Colourful buckets beat grey ones of the same size; near-black and near-white only
 * count when there is nothing else. Always returns `n` colours.
 */
export function palette(rgba: Uint8ClampedArray, n = 4): Rgb[] {
	const buckets = new Map<number, { r: number; g: number; b: number; count: number }>();
	for (let i = 0; i + 3 < rgba.length; i += 4) {
		if (rgba[i + 3] < 128) continue;
		const [r, g, b] = [rgba[i], rgba[i + 1], rgba[i + 2]];
		const key = ((r >> 4) << 8) | ((g >> 4) << 4) | (b >> 4);
		const bucket = buckets.get(key) ?? { r: 0, g: 0, b: 0, count: 0 };
		bucket.r += r;
		bucket.g += g;
		bucket.b += b;
		bucket.count++;
		buckets.set(key, bucket);
	}
	const candidates = [...buckets.values()].map(({ r, g, b, count }) => {
		const rgb: Rgb = [Math.round(r / count), Math.round(g / count), Math.round(b / count)];
		const [s, l] = satLight(rgb);
		const extreme = l < 0.08 || l > 0.94;
		return { rgb, count, extreme, score: count * (0.3 + s) };
	});
	candidates.sort((a, b) => b.score - a.score);

	const picked: Rgb[] = [];
	for (const { rgb } of candidates.filter((c) => !c.extreme)) {
		if (picked.length === n) break;
		if (picked.every((p) => distance(p, rgb) >= DISTINCT)) picked.push(rgb);
	}
	if (!picked.length) {
		const common = candidates.toSorted((a, b) => b.count - a.count)[0];
		picked.push(common?.rgb ?? [128, 128, 128]);
	}
	return spread(picked, n);
}

/** Pad `colors` to `n` with lighter and darker shades of the ones there. */
export function spread(colors: Rgb[], n: number): Rgb[] {
	const out = colors.slice(0, n);
	for (let i = 0; out.length < n; i++) {
		const base = colors[i % colors.length];
		const round = Math.floor(i / colors.length) + 1;
		const target = i % 2 ? 0 : 255;
		out.push(mix(base, [target, target, target], Math.min(0.6, 0.25 * round)));
	}
	return out;
}

export function mix(a: Rgb, b: Rgb, t: number): Rgb {
	return [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
}

/** CSS `saturate(amount)`: push each channel away from the luminance, clamped to 0–255. */
export function saturate(rgb: Rgb, amount: number): Rgb {
	const l = 0.213 * rgb[0] + 0.715 * rgb[1] + 0.072 * rgb[2];
	return rgb.map((c) => Math.min(255, Math.max(0, l + (c - l) * amount))) as Rgb;
}

/** sRGB to OKLab (L 0–1, a and b about ±0.4): blends in it stay as bright and vivid as their ends. */
export function oklab(rgb: Rgb): [number, number, number] {
	const [r, g, b] = rgb.map((c) => {
		const v = c / 255;
		return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
	});
	const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
	const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
	const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
	return [
		0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
		1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
		0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s
	];
}

/** `#rrggbb` (or `#rgb`) to RGB; null for anything else. */
export function hexRgb(hex: string): Rgb | null {
	const m = /^#([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(hex.trim());
	if (!m) return null;
	const h = m[1].length === 3 ? [...m[1]].map((c) => c + c).join('') : m[1];
	return [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16)) as Rgb;
}

/** HSL saturation and lightness, 0–1. */
function satLight([r, g, b]: Rgb): [number, number] {
	const max = Math.max(r, g, b) / 255;
	const min = Math.min(r, g, b) / 255;
	const l = (max + min) / 2;
	const d = max - min;
	return [d === 0 ? 0 : d / (1 - Math.abs(2 * l - 1)), l];
}

function distance(a: Rgb, b: Rgb): number {
	return Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);
}
