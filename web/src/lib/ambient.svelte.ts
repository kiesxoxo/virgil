// The colour behind every page (Backdrop.svelte, kept in the layout): soft fields in a cover's
// colours. A page says what it wants while it is on screen; between pages the colour carries
// over, blending from one page's to the next instead of starting again. Pages that want nothing
// let it fade away, and then nothing is drawn.

import { palette, saturate, type Rgb } from './palette.ts';

export interface Claim {
	/** Which page this is: the same page updating its claim (scrolling, say) doesn't start a new transition. */
	key: string;
	/** The image whose colours to show; null for the accent's. */
	src: string | null;
	/** 0 none … 1 full. */
	strength: number;
	/** How far down the screen the colour reaches (0–1); it fades out on the way. */
	reach: number;
	/** Drifting slowly (while playing). */
	drift: boolean;
	/** Where the colour grows from, in CSS pixels on the screen (and shrinks back to when it goes). */
	from?: { x: number; y: number } | null;
}

/** Colours a little richer than the cover's, so they hold up under the fade to the background. */
const SATURATE = 1.5;

class Ambient {
	/** What the page on screen wants, or null. */
	claim = $state.raw<Claim | null>(null);
	/** Where the next page's colour should grow from: the mini player's cover, on the way to Now Playing. */
	origin: { x: number; y: number } | null = null;

	/** Shows `claim` until the returned function is called (an `$effect`'s cleanup). */
	want(claim: Claim): () => void {
		this.claim = claim;
		return () => {
			if (this.claim === claim) this.claim = null;
		};
	}
}

export const ambient = new Ambient();

const cache = new Map<string, Promise<Rgb[] | null>>();

/** The colours of the image at `src`, or null for none (or one the canvas may not read). */
export function colorsOf(src: string): Promise<Rgb[] | null> {
	let colors = cache.get(src);
	if (!colors) {
		colors = new Promise((resolve) => {
			const img = new Image();
			// Same-origin covers always work; radio artwork only if its host sends CORS headers.
			img.crossOrigin = 'anonymous';
			img.onload = () => resolve(sample(img));
			img.onerror = () => resolve(null);
			img.src = src;
		});
		cache.set(src, colors);
		// A few dozen covers at most: forget the oldest.
		if (cache.size > 64) cache.delete(cache.keys().next().value!);
	}
	return colors;
}

function sample(img: HTMLImageElement): Rgb[] | null {
	const canvas = document.createElement('canvas');
	canvas.width = canvas.height = 24;
	const ctx = canvas.getContext('2d', { willReadFrequently: true });
	if (!ctx) return null;
	ctx.drawImage(img, 0, 0, 24, 24);
	try {
		return palette(ctx.getImageData(0, 0, 24, 24).data).map((c) => saturate(c, SATURATE));
	} catch {
		return null; // tainted
	}
}
