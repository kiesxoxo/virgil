import assert from 'node:assert/strict';
import { test } from 'node:test';
import { blocks } from './requests.ts';
import type { QueueItem } from './types/QueueItem.ts';

let id = 0;
const song = (by: string | null, request: number, albumId: string | null): QueueItem => ({
	id: id++,
	uri: `song${id}`,
	title: '',
	artist: '',
	album: '',
	duration: 60,
	format: null,
	albumId,
	art: null,
	pick: by ? { by, request, at: 0, note: null } : null
});

test('a request is one block, and an album request reads as the album', () => {
	const b = blocks([song('ola', 1, 'x'), song('ola', 1, 'x'), song('jan', 2, 'y'), song('ola', 3, 'x')]);
	assert.deepEqual(
		b.map((x) => [x.items.length, x.album, x.pick?.by]),
		[
			[2, true, 'ola'],
			[1, false, 'jan'],
			[1, false, 'ola']
		]
	);
});

test('songs nobody requested group by album, and songs from several albums are no album', () => {
	const b = blocks([song(null, 0, 'x'), song(null, 0, 'x'), song(null, 0, null), song(null, 0, null), song('kai', 4, 'x'), song('kai', 4, 'y')]);
	assert.deepEqual(
		b.map((x) => [x.items.length, x.album]),
		[
			[2, true],
			[1, false],
			[1, false],
			[2, false]
		]
	);
	assert.equal(new Set(b.map((x) => x.key)).size, b.length);
});
