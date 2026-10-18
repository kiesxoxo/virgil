import type { AudioFormat } from './types/AudioFormat.ts';

const pad = (n: number) => String(n).padStart(2, '0');

/** 3:07, or 1:02:03 past an hour. */
export function clock(seconds: number): string {
	const s = Number.isFinite(seconds) && seconds > 0 ? Math.floor(seconds) : 0;
	const h = Math.floor(s / 3600);
	const m = Math.floor((s % 3600) / 60);
	return h ? `${h}:${pad(m)}:${pad(s % 60)}` : `${m}:${pad(s % 60)}`;
}

/** "1 hr 12 min" / "43 min". */
export function span(seconds: number): string {
	const total = Math.round(seconds / 60);
	const h = Math.floor(total / 60);
	return h ? `${h} hr ${total % 60} min` : `${total} min`;
}

export function khz(rate: number): string {
	const k = rate / 1000;
	return Number.isInteger(k) ? String(k) : k.toFixed(1);
}

function dsdLabel(f: AudioFormat): string {
	return `DSD${Math.round(f.rate / 44_100)}`;
}

/** Compact: "24/96", "16/44.1", "DSD64". */
export function formatShort(f: AudioFormat | null | undefined): string {
	if (!f) return '';
	if (f.format === 'dsd') return dsdLabel(f);
	const bits = f.format === 'f32' ? '32f' : f.format.slice(1);
	return `${bits}/${khz(f.rate)}`;
}

/** Spelled out: "24-bit · 96 kHz", "32-bit float · 44.1 kHz", "DSD64 · 2.8 MHz". */
export function formatLong(f: AudioFormat | null | undefined): string {
	if (!f) return '';
	if (f.format === 'dsd') return `${dsdLabel(f)} · ${(f.rate / 1e6).toFixed(1)} MHz`;
	const bits = f.format === 'f32' ? '32-bit float' : `${f.format.slice(1)}-bit`;
	const channels = f.channels === 2 ? '' : f.channels === 1 ? ' · mono' : ` · ${f.channels} ch`;
	return `${bits} · ${khz(f.rate)} kHz${channels}`;
}

/** ALSA sample format as words: "S24_LE" → "24-bit". */
export function alsaFormat(format: string): string {
	const bits = /^[SU](\d+)/.exec(format)?.[1];
	if (bits) return `${bits}-bit`;
	if (format.startsWith('FLOAT')) return '32-bit float';
	return format;
}

const BITS = { s8: 8, s16: 16, s24: 24, s32: 32, f32: 32, dsd: 1 } as const;

/** Orders formats by resolution: DSD above PCM, PCM by rate × bits, unknown/mixed last. */
export function qualityRank(f: AudioFormat | null | undefined): number {
	if (!f) return 0;
	return f.format === 'dsd' ? 1e9 + f.rate : f.rate * BITS[f.format];
}

/** Better than CD: more than 16 bits or faster than 48 kHz. */
export function isHiRes(f: AudioFormat | null | undefined): boolean {
	if (!f) return false;
	return f.format === 'dsd' || f.format === 'f32' || f.format === 's24' || f.format === 's32' || f.rate > 48_000;
}

export function artUrl(art: string | null | undefined, size: 300 | 600 = 300): string | null {
	return art ? `/art/${art}-${size}.jpg` : null;
}

/** Lowercase and strip diacritics, for client-side matching ("Björk" ~ "bjork"). */
export function fold(s: string): string {
	return s.normalize('NFD').replace(/\p{Diacritic}/gu, '').toLowerCase();
}

/**
 * Words only, for telling titles and names apart: folded, with punctuation
 * and bracketed tags such as "[WEB]", "(2014)" or "(Remastered)" dropped.
 * "The Wall (Remastered)" and "the wall" both give "the wall".
 */
export function bare(s: string): string {
	return fold(s)
		.replace(/[([{][^)\]}]*[)\]}]/g, ' ')
		.replace(/[^\p{L}\p{N}]+/gu, ' ')
		.trim();
}
