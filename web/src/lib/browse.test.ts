// node --test src/lib/browse.test.ts

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { appearsOn, artistParts, artists, find, genres, NO_GENRE_NAME, splitGenres } from './browse.ts';
import type { Album } from './types/Album.ts';

let n = 0;
const album = (artist: string, genre: string | null, year: number | null = null, title = `Album ${n}`): Album => ({
	id: String(n++),
	title,
	artist,
	year,
	genre,
	tracks: 1,
	duration: 60,
	art: null,
	format: null,
	added: 0
});

test('splitGenres', () => {
	assert.deepEqual(splitGenres('Electronic / Ambient / Drone'), ['Electronic', 'Ambient', 'Drone']);
	assert.deepEqual(splitGenres('Ambient, Tribal, Field Recording'), ['Ambient', 'Tribal', 'Field Recording']);
	assert.deepEqual(splitGenres('Electro; Électronique'), ['Electro', 'Électronique']);
	assert.deepEqual(splitGenres('Techno; Techno (Peak Time / Driving)'), ['Techno', 'Techno (Peak Time / Driving)']);
	assert.deepEqual(splitGenres(null), []);
	assert.deepEqual(splitGenres('  '), []);
});

test('artistParts', () => {
	assert.deepEqual(artistParts('Daniel Avery & Alessandro Cortini'), ['Daniel Avery', 'Alessandro Cortini']);
	assert.deepEqual(artistParts('HKE x Sangam'), ['HKE', 'Sangam']);
	assert.deepEqual(artistParts('Hwang Joo-eun, Oliver Good, Keita Inoue'), ['Hwang Joo-eun', 'Oliver Good', 'Keita Inoue']);
	assert.deepEqual(artistParts('A feat. B'), ['A', 'B']);
	// "x" only as a word, not inside names.
	assert.deepEqual(artistParts('Xiu Xiu'), ['Xiu Xiu']);
});

test('genres merge spellings, split tags and keep untagged albums last', () => {
	const list = [
		album('A', 'Electronic'),
		album('B', 'electronic'),
		album('C', 'ELECTRONIC, Ambient'),
		album('D', null),
		album('E', 'Ambient')
	];
	const g = genres(list);
	assert.deepEqual(
		g.map((x) => [x.name, x.albums.length]),
		[
			['Ambient', 2],
			['Electronic', 3],
			[NO_GENRE_NAME, 1]
		]
	);
	assert.equal(find(g, 'ELECTRONIC')?.name, 'Electronic');
	assert.equal(find(g, NO_GENRE_NAME)?.albums[0].artist, 'D');
});

test('artists: one entry per album artist, albums oldest first; collaborations appear on members', () => {
	const list = [
		album('Alessandro Cortini', null, 2017, 'Avanti'),
		album('alessandro cortini', null, 2014, 'Forse 1'),
		album('Daniel Avery & Alessandro Cortini', null, 2020, 'Illusion of Time'),
		album('Autechre', null, 1994)
	];
	const a = artists(list);
	assert.deepEqual(a.map((x) => x.name), ['Alessandro Cortini', 'Autechre', 'Daniel Avery & Alessandro Cortini']);
	assert.deepEqual(a[0].albums.map((x) => x.title), ['Forse 1', 'Avanti']);
	assert.deepEqual(appearsOn(list, a[0].key).map((x) => x.title), ['Illusion of Time']);
	assert.equal(find(a, 'ALESSANDRO CORTINI')?.albums.length, 2);

	// The same album also tagged under the member's name is listed once, under their albums.
	const both = [...list, album('Alessandro Cortini', null, 2020, 'Illusion Of Time [WEB]')];
	assert.deepEqual(appearsOn(both, a[0].key), []);
});
