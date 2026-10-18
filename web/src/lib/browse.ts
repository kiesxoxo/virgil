// Artists and genres, worked out from the album list. Kept free of Svelte so
// it can be tested on its own (browse.test.ts).

import { fold } from './format.ts';
import type { Album } from './types/Album.ts';

export interface Group {
	/** Folded name: what URLs and lookups match on. */
	key: string;
	/** The spelling most albums use. */
	name: string;
	/** Albums in this group, oldest first (then by title). */
	albums: Album[];
}

/** The key of the genre bucket for albums without one. */
export const NO_GENRE = '';
export const NO_GENRE_NAME = 'No genre';

const key = (s: string) => fold(s).replace(/\s+/g, ' ').trim();

/**
 * The genres in one tag: "Electronic / Ambient", "Ambient, Drone" and
 * "Electro; Électronique" each hold two. Separators inside parentheses don't
 * count: "Techno (Peak Time / Driving)" is one genre.
 */
export function splitGenres(tag: string | null): string[] {
	if (!tag) return [];
	const parts: string[] = [];
	let depth = 0;
	let current = '';
	for (const c of tag) {
		if (c === '(' || c === '[') depth++;
		else if ((c === ')' || c === ']') && depth > 0) depth--;
		if (depth === 0 && (c === ',' || c === ';' || c === '/')) {
			parts.push(current);
			current = '';
		} else current += c;
	}
	parts.push(current);
	return parts.map((p) => p.trim()).filter(Boolean);
}

/**
 * The artists behind a collaboration's name: "Daniel Avery & Alessandro
 * Cortini", "HKE x Sangam", "A feat. B" or "A, B, C".
 */
export function artistParts(artist: string): string[] {
	return artist
		.split(/\s*(?:,|;|&|\+|\bx\b|\bfeat\.?|\bft\.?|\bfeaturing\b|\bwith\b|\bvs\.?)\s*/i)
		.map((p) => p.trim())
		.filter(Boolean);
}

/** The best display spelling: the most used, preferring mixed case over "ELECTRONIC" or "electronic". */
function spelling(counts: Map<string, number>): string {
	const shouty = (s: string) => s === s.toUpperCase() || s === s.toLowerCase();
	return [...counts].sort(([a, n], [b, m]) => m - n || Number(shouty(a)) - Number(shouty(b)) || a.localeCompare(b))[0][0];
}

const collator = new Intl.Collator(undefined, { sensitivity: 'base', numeric: true });

function chronological(a: Album, b: Album): number {
	return (a.year ?? 9999) - (b.year ?? 9999) || collator.compare(a.title, b.title);
}

/** Gathers albums under keys, with the most used spelling of each name. */
function groupBy(albums: Album[], names: (album: Album) => string[]): Map<string, { names: Map<string, number>; albums: Album[] }> {
	const groups = new Map<string, { names: Map<string, number>; albums: Album[] }>();
	for (const album of albums) {
		const seen = new Set<string>();
		for (const name of names(album)) {
			const k = key(name);
			if (seen.has(k)) continue;
			seen.add(k);
			let group = groups.get(k);
			if (!group) groups.set(k, (group = { names: new Map(), albums: [] }));
			group.names.set(name, (group.names.get(name) ?? 0) + 1);
			group.albums.push(album);
		}
	}
	return groups;
}

/** One entry per album artist, A–Z. */
export function artists(albums: Album[]): Group[] {
	const groups = groupBy(albums, (a) => [a.artist.trim() || 'Unknown Artist']);
	return [...groups]
		.map(([k, g]) => ({ key: k, name: spelling(g.names), albums: g.albums.toSorted(chronological) }))
		.sort((a, b) => collator.compare(a.name, b.name));
}

/** A title without case, accents, punctuation or bracketed tags like "[WEB]". */
const titleKey = (title: string) =>
	fold(title)
		.replace(/[([{][^)\]}]*[)\]}]/g, ' ')
		.replace(/[^\p{L}\p{N}]+/gu, '');

/**
 * Albums an artist is part of under another name: "Daniel Avery & Alessandro
 * Cortini" for Alessandro Cortini. A copy of one of their own albums, tagged
 * with the collaboration's name instead, isn't listed twice.
 */
export function appearsOn(albums: Album[], artistKey: string): Album[] {
	const own = new Set(albums.filter((a) => key(a.artist) === artistKey).map((a) => titleKey(a.title)));
	return albums
		.filter((a) => key(a.artist) !== artistKey && artistParts(a.artist).some((p) => key(p) === artistKey))
		.filter((a) => !own.has(titleKey(a.title)))
		.toSorted(chronological);
}

/** One entry per genre, A–Z, with "No genre" last. An album with several genres is in each. */
export function genres(albums: Album[]): Group[] {
	const groups = groupBy(albums, (a) => splitGenres(a.genre));
	const list = [...groups]
		.map(([k, g]) => ({ key: k, name: spelling(g.names), albums: g.albums }))
		.sort((a, b) => collator.compare(a.name, b.name));
	const untagged = albums.filter((a) => splitGenres(a.genre).length === 0);
	if (untagged.length) list.push({ key: NO_GENRE, name: NO_GENRE_NAME, albums: untagged });
	return list;
}

/** Looks a group up by the name in a URL, which may differ in case or accents. */
export function find(groups: Group[], name: string): Group | undefined {
	const k = name === NO_GENRE_NAME ? NO_GENRE : key(name);
	return groups.find((g) => g.key === k);
}

/** Up to four covers to show for a group: different albums, ones with art first. */
export function covers(group: Group): (string | null)[] {
	const withArt = group.albums.filter((a) => a.art);
	return [...withArt, ...group.albums.filter((a) => !a.art)].slice(0, 4).map((a) => a.art);
}
