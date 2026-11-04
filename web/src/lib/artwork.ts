// Which covers the browser has decoded lately. A cover coming onto the page shows the sharpest of
// those at once, while its own size comes in, rather than a placeholder: switching views shows
// every cover from the start.

import { artUrl, type ArtSize } from './format.ts';

/** Sharpest first. */
const SIZES: ArtSize[] = [1000, 600, 300];
/** About as many as a few screens of covers in every size: the browser may have let go of older ones. */
const KEEP = 300;

/** Decoded images by path (an `<img>`'s currentSrc is a full URL, artUrl's a path), the most recent last. */
const decoded = new Set<string>();

const path = (url: string) => (url.startsWith('/') ? url : new URL(url).pathname);

/** Notes that the image at `url` has been decoded. */
export function markDecoded(url: string | null | undefined) {
	if (!url) return;
	const key = path(url);
	decoded.delete(key);
	decoded.add(key);
	if (decoded.size > KEEP) decoded.delete(decoded.values().next().value!);
}

export function isDecoded(url: string | null | undefined): boolean {
	return !!url && decoded.has(path(url));
}

/** The sharpest size of cover `art` decoded lately, other than `except`: one to show while that one comes in. */
export function standIn(art: string | null | undefined, except?: string | null): string | null {
	for (const size of SIZES) {
		const url = artUrl(art, size);
		if (url && url !== except && decoded.has(url)) return url;
	}
	return null;
}
