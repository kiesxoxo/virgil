// The tag editor's logic, modelled on foobar2000's Properties dialog. Kept
// free of Svelte so it can be tested on its own (tagedit.test.ts).

import type { LookupRelease } from './types/LookupRelease.ts';
import type { LookupTrack } from './types/LookupTrack.ts';
import type { TagEdit } from './types/TagEdit.ts';
import type { TaggedFile } from './types/TaggedFile.ts';

/** The fields foobar2000 always lists first, with their friendly names. */
export const STANDARD: [string, string][] = [
	['ARTIST', 'Artist Name'],
	['TITLE', 'Track Title'],
	['ALBUM', 'Album Title'],
	['DATE', 'Date'],
	['GENRE', 'Genre'],
	['COMPOSER', 'Composer'],
	['PERFORMER', 'Performer'],
	['ALBUM ARTIST', 'Album Artist'],
	['TRACKNUMBER', 'Track Number'],
	['TOTALTRACKS', 'Total Tracks'],
	['DISCNUMBER', 'Disc Number'],
	['TOTALDISCS', 'Total Discs'],
	['COMMENT', 'Comment']
];

/** Other names files use for standard fields; shown in the standard row, written back under the file's own name. */
export const ALIASES: Record<string, string[]> = {
	'ALBUM ARTIST': ['ALBUMARTIST', 'ALBUM_ARTIST'],
	TOTALTRACKS: ['TRACKTOTAL'],
	TOTALDISCS: ['DISCTOTAL']
};

/** Fields whose "a; b" text means several values (foobar2000's default list). */
export const MULTIVALUE = new Set(['ARTIST', 'ALBUM ARTIST', 'PRODUCER', 'COMPOSER', 'PERFORMER', 'GENRE']);

/** Fields that get a multi-line box. */
export const LONG = new Set(['COMMENT', 'DESCRIPTION', 'LYRICS', 'UNSYNCED LYRICS', 'UNSYNCEDLYRICS']);

export const SEPARATOR = '; ';

export interface Row {
	/** Unique and stable while the editor is open. */
	id: number;
	/** The name to write. */
	name: string;
	/** The name the row opened with, undefined for a field added here. */
	original?: string;
	/** Per file: the field's name in that file (its own spelling, or an alias), if it has the field. */
	names: (string | undefined)[];
	/** Friendly label for standard fields. */
	label?: string;
	/** Per file (same order as the editor's files): the text shown, values joined by "; ". */
	text: string[];
	/** Per file: the values as read. */
	before: string[][];
	removed: boolean;
}

let ids = 0;

const upper = (name: string) => name.toUpperCase();

/**
 * One row per field across `files`: standard fields first (each file's alias
 * standing in when it lacks the standard name), then the rest A–Z.
 */
export function buildRows(files: TaggedFile[]): Row[] {
	// Per file, the fields (by upper-case name) already shown in a row.
	const used = files.map(() => new Set<string>());
	const row = (name: string, names: (string | undefined)[], label?: string): Row => {
		const before = files.map((f, i) => f.fields.find((field) => field.name === names[i])?.values ?? []);
		return {
			id: ids++,
			name,
			original: names.some(Boolean) ? name : undefined,
			names,
			label,
			text: before.map((v) => v.join(SEPARATOR)),
			before,
			removed: false
		};
	};
	const claim = (i: number, candidates: string[]) => {
		for (const candidate of candidates) {
			const field = files[i].fields.find((f) => upper(f.name) === candidate);
			if (field && !used[i].has(candidate)) {
				used[i].add(candidate);
				return field.name;
			}
		}
		return undefined;
	};
	const standard = STANDARD.map(([key, label]) => row(key, files.map((_, i) => claim(i, [key, ...(ALIASES[key] ?? [])])), label));
	// The remaining fields, named as the first file that has them spells them.
	const spelling = new Map<string, string>();
	files.forEach((file, i) => {
		for (const field of file.fields) {
			const key = upper(field.name);
			if (!used[i].has(key) && !spelling.has(key)) spelling.set(key, field.name);
		}
	});
	const others = [...spelling.keys()].sort().map((key) => row(spelling.get(key)!, files.map((_, i) => claim(i, [key]))));
	return [...standard, ...others];
}

export function newRow(name: string, count: number): Row {
	return {
		id: ids++,
		name,
		names: Array(count).fill(undefined),
		text: Array(count).fill(''),
		before: Array(count).fill([]),
		removed: false
	};
}

/** The shared text, or null when files differ ("<multiple values>"). */
export function shared(row: Row): string | null {
	return row.text.every((t) => t === row.text[0]) ? (row.text[0] ?? '') : null;
}

export function split(name: string, text: string): string[] {
	const values = MULTIVALUE.has(upper(name)) ? text.split(';') : [text];
	return values.map((v) => v.trim()).filter(Boolean);
}

function renamed(row: Row): boolean {
	return row.original !== undefined && upper(row.name) !== upper(row.original);
}

export function changed(row: Row): boolean {
	if (row.removed) return row.before.some((v) => v.length > 0);
	return renamed(row) || row.text.some((t, i) => t !== row.before[i].join(SEPARATOR));
}

/** What to write to each file; files with nothing to change are left out. */
export function edits(files: TaggedFile[], rows: Row[]): TagEdit[] {
	return files.flatMap((file, i) => {
		const edit: TagEdit = { uri: file.uri, set: [], remove: [] };
		for (const row of rows) {
			const had = row.names[i];
			if (row.removed) {
				if (had) edit.remove.push(had);
				continue;
			}
			const textChanged = row.text[i] !== row.before[i].join(SEPARATOR);
			if (renamed(row)) {
				if (had) edit.remove.push(had);
				const values = textChanged ? split(row.name, row.text[i]) : row.before[i];
				if (values.length) edit.set.push({ name: row.name, values });
			} else if (textChanged) {
				// Keep the file's own name for the field (e.g. ALBUMARTIST).
				edit.set.push({ name: had ?? row.name, values: split(row.name, row.text[i]) });
			}
		}
		return edit.set.length || edit.remove.length ? [edit] : [];
	});
}

/** A field name every format can store (Vorbis comments' rule). */
export function validName(name: string): boolean {
	return /^[\x20-\x3C\x3E-\x7D]{1,255}$/.test(name);
}

// --- Tools ---------------------------------------------------------------

export type Case = 'title' | 'upper' | 'lower' | 'trim';

const SMALL = new Set(['a', 'an', 'and', 'as', 'at', 'but', 'by', 'for', 'in', 'nor', 'of', 'on', 'or', 'the', 'to', 'vs', 'via']);

export function recase(text: string, mode: Case): string {
	switch (mode) {
		case 'upper':
			return text.toUpperCase();
		case 'lower':
			return text.toLowerCase();
		case 'trim':
			return text.replace(/\s+/g, ' ').trim();
		case 'title': {
			const words = text.split(/(\s+)/);
			return words
				.map((word, i) => {
					if (/^\s+$/.test(word) || !word) return word;
					const lower = word.toLowerCase();
					const edge = i === 0 || i === words.length - 1 || /[:\-–—(]$/.test(words[i - 2] ?? '');
					if (!edge && SMALL.has(lower)) return lower;
					// Leave words with inner capitals (iPhone, McCartney, AC/DC) alone.
					if (/\p{Lu}/u.test(word.slice(1))) return word;
					return lower.replace(/^(\P{L}*)(\p{L})/u, (_, lead: string, c: string) => lead + c.toUpperCase());
				})
				.join('');
		}
	}
}

/** Numbers files 1…n in the given order, like foobar2000's Auto Track Number. */
export function autoNumber(count: number): { number: string[]; total: string } {
	const width = count >= 10 ? 2 : 1;
	return { number: Array.from({ length: count }, (_, i) => String(i + 1).padStart(width, '0')), total: String(count) };
}

/**
 * Fields from a file name pattern like "%tracknumber% - %title%", matched
 * against the end of the path without its extension, so "%album%/%title%"
 * reads the folder too. Returns null for a file that doesn't match.
 */
export function fillFromName(pattern: string, uri: string): Record<string, string> | null {
	const parts = pattern.split(/(%[^%]+%)/).filter(Boolean);
	const fields: string[] = [];
	let source = '';
	for (const part of parts) {
		const field = /^%([^%]+)%$/.exec(part);
		if (field) {
			fields.push(field[1].toUpperCase());
			source += '([^/]*?)';
		} else {
			source += part.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
		}
	}
	if (!fields.length) return null;
	const path = uri.replace(/\.[^./]+$/, '');
	const match = new RegExp(`(?:^|/)${source}$`, 'u').exec(path);
	if (!match) return null;
	const out: Record<string, string> = {};
	fields.forEach((name, i) => {
		// %_% (or anything starting with "_") skips that part, as in foobar2000.
		if (!name.startsWith('_')) out[name] = match[i + 1].trim();
	});
	return out;
}

// --- Online lookup -------------------------------------------------------

/** What a looked-up release fills in: per field, the text for each file, or null to leave that file's value. */
export interface ReleaseFill {
	fields: [string, (string | null)[]][];
	/** Reasons to check the release is the right one before saving. */
	warnings: string[];
}

/** Seconds two lengths may differ by and still be the same recording. */
const SAME_LENGTH = 3;

/** A file's number field ("3", "03", "3/12"), or null. */
function numberOf(file: TaggedFile, names: string[]): number | null {
	const value = file.fields.find((f) => names.includes(upper(f.name)))?.values[0] ?? '';
	const n = parseInt(value, 10);
	return Number.isFinite(n) && n > 0 ? n : null;
}

/**
 * The fields a release sets. Tracks go to files by disc and track number
 * when every file has a number the release lists, else in album order.
 * Album-wide fields go to every file; track fields only to matched files.
 */
export function fromRelease(files: TaggedFile[], release: LookupRelease): ReleaseFill {
	const tracks = release.tracks;
	const numbered = files.map((file) => {
		const number = numberOf(file, ['TRACKNUMBER', 'TRACK']);
		const disc = numberOf(file, ['DISCNUMBER', 'DISC']) ?? 1;
		return number === null ? undefined : tracks.find((t) => t.disc === disc && t.number === number);
	});
	const byNumber = numbered.every(Boolean) && new Set(numbered).size === files.length;
	const matched: (LookupTrack | undefined)[] = byNumber ? numbered : files.map((_, i) => tracks[i]);

	const discs = new Set(tracks.map((t) => t.disc)).size;
	const onDisc = (disc: number) => tracks.filter((t) => t.disc === disc).length;
	const all = (text: string) => files.map(() => text);
	const each = (text: (t: LookupTrack) => string) => matched.map((t) => (t ? text(t) : null));

	const fields: [string, (string | null)[]][] = [];
	if (release.title) fields.push(['ALBUM', all(release.title)]);
	if (release.artist) fields.push(['ALBUM ARTIST', all(release.artist)]);
	fields.push(['ARTIST', each((t) => t.artist)], ['TITLE', each((t) => t.title)]);
	if (release.date) fields.push(['DATE', all(release.date)]);
	if (release.originalDate && release.originalDate !== release.date) fields.push(['ORIGINALDATE', all(release.originalDate)]);
	if (release.genres.length) fields.push(['GENRE', all(release.genres.join(SEPARATOR))]);
	fields.push(
		['TRACKNUMBER', each((t) => String(t.number).padStart(onDisc(t.disc) >= 10 ? 2 : 1, '0'))],
		['TOTALTRACKS', each((t) => String(onDisc(t.disc)))]
	);
	if (discs > 1) fields.push(['DISCNUMBER', each((t) => String(t.disc))], ['TOTALDISCS', all(String(discs))]);

	const warnings: string[] = [];
	if (tracks.length !== files.length) {
		warnings.push(`The release has ${tracks.length} tracks for ${files.length} ${files.length === 1 ? 'file' : 'files'}: check they line up.`);
	}
	const off = matched.filter((t, i) => {
		const length = files[i].info?.duration;
		return t?.duration != null && length != null && Math.abs(t.duration - length) > SAME_LENGTH;
	}).length;
	if (off) {
		warnings.push(`${off} ${off === 1 ? 'track differs' : 'tracks differ'} in length by more than ${SAME_LENGTH} seconds: is this the right release?`);
	}
	return { fields, warnings };
}
