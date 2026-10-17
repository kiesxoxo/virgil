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
