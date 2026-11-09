import assert from 'node:assert/strict';
import { test } from 'node:test';
import { awards, setList, setListText, summary } from './aircheck.ts';
import type { Aircheck } from './types/Aircheck.ts';
import type { Played } from './types/Played.ts';

const person = (id: string, name: string) => ({ id, name, sign: name.slice(0, 3).toUpperCase(), color: 0, host: false });

const played = (title: string, by: string | null, request: number, at: number, extra: Partial<Played> = {}): Played => ({
	uri: title,
	title,
	artist: 'Artist',
	album: 'Album',
	albumId: 'x',
	art: null,
	duration: 600,
	at,
	pick: by ? { by, request, at, note: null } : null,
	cheers: 0,
	skipVotes: 0,
	skipped: false,
	...extra
});

const night: Aircheck = {
	opened: 1000,
	closed: 1000 + 3 * 3600,
	people: [person('o', 'Ola'), person('j', 'Jan')],
	played: [
		played('One', 'o', 1, 1100),
		played('Two', 'o', 1, 1100),
		played('Three', 'j', 2, 2000, { cheers: 3 }),
		played('Four', null, 0, 3000),
		played('Five', 'j', 3, 4000, { skipVotes: 2, skipped: true })
	]
};

test('the set list keeps requests together', () => {
	const list = setList(night);
	assert.deepEqual(
		list.map((e) => [e.songs.length, e.album, e.person?.name ?? null]),
		[
			[2, true, 'Ola'],
			[1, false, 'Jan'],
			[1, false, null],
			[1, false, 'Jan']
		]
	);
});

test('awards go to who earned them', () => {
	const won = Object.fromEntries(awards(night).map((a) => [a.title, `${a.person?.name}: ${a.detail}`]));
	assert.deepEqual(won, {
		'First caller': 'Ola: One',
		'Most requests': 'Jan: 2 requests',
		'Crowd favourite': 'Jan: Three · ♥ 3',
		'Longest set': 'Ola: 20 min',
		'Bravest pick': 'Jan: Five · 2 skip votes'
	});
});

test('a quiet night gives no awards for nothing', () => {
	const quiet: Aircheck = { opened: 0, closed: null, people: [person('o', 'Ola')], played: [played('One', 'o', 1, 10)] };
	assert.deepEqual(
		awards(quiet).map((a) => a.title),
		['First caller']
	);
	assert.equal(summary(quiet), '1 caller · 1 song');
});

test('the set list reads as text', () => {
	const text = setListText(night, (t) => String(t));
	assert.match(text, /^Aircheck · 2 callers · 5 songs · 3 hr 0 min/);
	assert.match(text, /1100 {2}Artist — Album \(album, 2 songs\) · Ola/);
	assert.match(text, /Bravest pick: Jan/);
});
