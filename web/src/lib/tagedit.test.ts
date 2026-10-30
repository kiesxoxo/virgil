// node --test src/lib/tagedit.test.ts

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { autoNumber, buildRows, edits, fillFromName, fromRelease, newRow, recase, shared, type Row } from './tagedit.ts';
import type { LookupRelease } from './types/LookupRelease.ts';
import type { TaggedFile } from './types/TaggedFile.ts';

const file = (uri: string, fields: Record<string, string[]>): TaggedFile => ({
	uri,
	fields: Object.entries(fields).map(([name, values]) => ({ name, values })),
	info: null,
	error: null
});

const files = [
	file('A/01 - One.flac', { ARTIST: ['X', 'Y'], TITLE: ['One'], ALBUM: ['A'], MOOD: ['calm'], Description: ['a; b'] }),
	file('A/02 - Two.flac', { ARTIST: ['X', 'Y'], TITLE: ['Two'], ALBUM: ['A'] })
];
const row = (rows: Row[], name: string) => rows.find((r) => r.name.toUpperCase() === name)!;

test('rows: standard fields first, shared and multiple values', () => {
	const rows = buildRows(files);
	assert.equal(rows[0].name, 'ARTIST');
	assert.equal(shared(row(rows, 'ARTIST')), 'X; Y');
	assert.equal(shared(row(rows, 'TITLE')), null);
	assert.equal(shared(row(rows, 'MOOD')), null, 'present in one file only');
	assert.deepEqual(rows.slice(13).map((r) => r.name), ['Description', 'MOOD']);
	assert.deepEqual(edits(files, rows), [], 'nothing changed');
});

test('edits: shared value, split multivalue, keep untouched values', () => {
	const rows = buildRows(files);
	row(rows, 'ARTIST').text = ['X; Z', 'X; Z'];
	row(rows, 'ALBUM').text = ['B', 'B'];
	assert.deepEqual(edits(files, rows), [
		{ uri: files[0].uri, set: [{ name: 'ARTIST', values: ['X', 'Z'] }, { name: 'ALBUM', values: ['B'] }], remove: [] },
		{ uri: files[1].uri, set: [{ name: 'ARTIST', values: ['X', 'Z'] }, { name: 'ALBUM', values: ['B'] }], remove: [] }
	]);
});

test('edits: semicolons stay in fields that are not multivalue', () => {
	const rows = buildRows(files);
	row(rows, 'DESCRIPTION').text = ['a; b; c', ''];
	assert.deepEqual(edits(files, rows)[0].set, [{ name: 'Description', values: ['a; b; c'] }]);
});

test('edits: rename keeps each file its own values, remove and add', () => {
	const rows = buildRows(files);
	row(rows, 'TITLE').name = 'WORK';
	row(rows, 'MOOD').removed = true;
	const added = newRow('MY FIELD', 2);
	added.text = ['1', '1'];
	rows.push(added);
	assert.deepEqual(edits(files, rows), [
		{ uri: files[0].uri, set: [{ name: 'WORK', values: ['One'] }, { name: 'MY FIELD', values: ['1'] }], remove: ['TITLE', 'MOOD'] },
		{ uri: files[1].uri, set: [{ name: 'WORK', values: ['Two'] }, { name: 'MY FIELD', values: ['1'] }], remove: ['TITLE'] }
	]);
});

test('edits: clearing a field removes it', () => {
	const rows = buildRows(files);
	row(rows, 'ALBUM').text = ['', ''];
	assert.deepEqual(edits(files, rows)[0], { uri: files[0].uri, set: [{ name: 'ALBUM', values: [] }], remove: [] });
});

test('aliases fill standard rows and are written back under their own name', () => {
	const mixed = [file('a.flac', { ALBUMARTIST: ['X'], TRACKTOTAL: ['9'] }), file('b.flac', { 'ALBUM ARTIST': ['X'], ALBUMARTIST: ['old'] })];
	const rows = buildRows(mixed);
	assert.equal(shared(row(rows, 'ALBUM ARTIST')), 'X');
	assert.equal(row(rows, 'TOTALTRACKS').text[0], '9');
	// b.flac has both names: the alias it doesn't need stays in its own row.
	assert.deepEqual(row(rows, 'ALBUMARTIST').text, ['', 'old']);
	row(rows, 'ALBUM ARTIST').text = ['Y', 'Y'];
	assert.deepEqual(edits(mixed, rows), [
		{ uri: 'a.flac', set: [{ name: 'ALBUMARTIST', values: ['Y'] }], remove: [] },
		{ uri: 'b.flac', set: [{ name: 'ALBUM ARTIST', values: ['Y'] }], remove: [] }
	]);
});

test('recase', () => {
	assert.equal(recase('the dark side of the moon', 'title'), 'The Dark Side of the Moon');
	assert.equal(recase('live at the BBC: the best of', 'title'), 'Live at the BBC: The Best Of');
	assert.equal(recase('  a   b ', 'trim'), 'a b');
	assert.equal(recase('Ünïcode', 'upper'), 'ÜNÏCODE');
});

test('autoNumber pads like the album has 10+ tracks', () => {
	assert.deepEqual(autoNumber(3), { number: ['1', '2', '3'], total: '3' });
	assert.deepEqual(autoNumber(12).number.slice(0, 2), ['01', '02']);
});

test('fillFromName', () => {
	assert.deepEqual(fillFromName('%tracknumber% - %title%', 'Some Album/01 - Hello - World.flac'), {
		TRACKNUMBER: '01',
		TITLE: 'Hello - World'
	});
	assert.deepEqual(fillFromName('%album%/%tracknumber%. %title%', 'Music/In Rainbows/03. Nude.flac'), {
		ALBUM: 'In Rainbows',
		TRACKNUMBER: '03',
		TITLE: 'Nude'
	});
	assert.deepEqual(fillFromName('%_% - %title%', 'x/Artist - Song.mp3'), { TITLE: 'Song' });
	assert.equal(fillFromName('%tracknumber% - %title%', 'x/NoDash.flac'), null);
});

const release = (tracks: [number, number, string, number | null][], extra: Partial<LookupRelease> = {}): LookupRelease => ({
	source: 'musicbrainz',
	id: 'x',
	title: 'Untrue',
	artist: 'Burial',
	date: '2007-11-05',
	originalDate: null,
	genres: ['Dubstep', 'Future Garage'],
	url: 'https://musicbrainz.org/release/x',
	tracks: tracks.map(([disc, number, title, duration]) => ({ disc, number, title, artist: 'Burial', duration })),
	...extra
});
const timed = (uri: string, fields: Record<string, string[]>, duration: number): TaggedFile => ({
	...file(uri, fields),
	info: { codec: 'FLAC', tagType: 'Vorbis comments', sampleRate: 44100, bits: 16, channels: 2, duration, bitrate: null, size: 1 }
});
const fill = (f: ReturnType<typeof fromRelease>, name: string) => f.fields.find(([n]) => n === name)?.[1];

test('release: by track number when every file has one, album fields everywhere', () => {
	const album = [timed('u/b.flac', { TRACKNUMBER: ['2'] }, 238), timed('u/a.flac', { TRACKNUMBER: ['01/13'] }, 50)];
	const f = fromRelease(album, release([[1, 1, '[untitled]', 50], [1, 2, 'Archangel', 238.2]]));
	assert.deepEqual(fill(f, 'TITLE'), ['Archangel', '[untitled]']);
	assert.deepEqual(fill(f, 'ALBUM ARTIST'), ['Burial', 'Burial']);
	assert.deepEqual(fill(f, 'GENRE'), ['Dubstep; Future Garage', 'Dubstep; Future Garage']);
	assert.deepEqual(fill(f, 'TOTALTRACKS'), ['2', '2']);
	assert.equal(fill(f, 'DISCNUMBER'), undefined, 'single disc: no disc fields');
	assert.equal(fill(f, 'ORIGINALDATE'), undefined, 'same as the date');
	assert.deepEqual(f.warnings, []);
});

test('release: in album order without numbers; counts and lengths warned about', () => {
	const album = [timed('u/1.flac', {}, 50), timed('u/2.flac', {}, 300), timed('u/3.flac', {}, 10)];
	const f = fromRelease(album, release([[1, 1, 'One', 50], [1, 2, 'Two', 238]], { originalDate: '2007' }));
	assert.deepEqual(fill(f, 'TITLE'), ['One', 'Two', null], 'the third file has no track');
	assert.deepEqual(fill(f, 'ALBUM'), ['Untrue', 'Untrue', 'Untrue']);
	assert.deepEqual(fill(f, 'ORIGINALDATE'), ['2007', '2007', '2007']);
	assert.equal(f.warnings.length, 2);
	assert.match(f.warnings[0], /2 tracks for 3 files/);
	assert.match(f.warnings[1], /1 track differs/);
});

test('release: several discs, numbers padded per disc', () => {
	const tracks: [number, number, string, number | null][] = [
		...Array.from({ length: 10 }, (_, i): [number, number, string, number | null] => [1, i + 1, `A${i + 1}`, null]),
		[2, 1, 'B1', null]
	];
	const album = tracks.map((_, i) => file(`u/${i}.flac`, {}));
	const f = fromRelease(album, release(tracks));
	assert.deepEqual(fill(f, 'TRACKNUMBER')?.slice(0, 2), ['01', '02']);
	assert.deepEqual(fill(f, 'TRACKNUMBER')?.at(-1), '1');
	assert.deepEqual(fill(f, 'TOTALTRACKS')?.at(-1), '1');
	assert.deepEqual(fill(f, 'DISCNUMBER')?.at(-1), '2');
	assert.deepEqual(fill(f, 'TOTALDISCS')?.[0], '2');
});

test('release: filled rows keep the file’s own field names when saved', () => {
	const album = [file('u/1.flac', { ALBUMARTIST: ['old'], TRACKTOTAL: ['9'] })];
	const rows = buildRows(album);
	const f = fromRelease(album, release([[1, 1, 'One', null]]));
	for (const [name, values] of f.fields) {
		const target = row(rows, name) ?? rows[rows.push(newRow(name, 1)) - 1];
		values.forEach((v, i) => v !== null && (target.text[i] = v));
	}
	const set = edits(album, rows)[0].set;
	assert.deepEqual(set.find((s) => s.name === 'ALBUMARTIST')?.values, ['Burial']);
	assert.deepEqual(set.find((s) => s.name === 'TRACKTOTAL')?.values, ['1']);
	assert.deepEqual(set.find((s) => s.name === 'GENRE')?.values, ['Dubstep', 'Future Garage']);
});
