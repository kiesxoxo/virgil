// node --test src/lib/fetching.test.ts

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { badge, describe, finished, latest, retryable } from './fetching.ts';

const album = { kind: 'album' as const, progress: null, note: null };

test('badge: a word or two for each state', () => {
	assert.equal(badge({ ...album, state: 'searching' }), 'Searching');
	assert.equal(badge({ ...album, state: 'downloading', progress: 0.437 }), '43%');
	assert.equal(badge({ ...album, state: 'downloading' }), 'Downloading');
	assert.equal(badge({ ...album, state: 'done' }), 'In library');
	assert.equal(badge({ ...album, state: 'notFound' }), 'Not found');
});

test('describe: the note when there is one, else what is going on', () => {
	assert.equal(describe({ ...album, state: 'searching' }), 'Lidarr is looking for a release');
	assert.equal(describe({ ...album, kind: 'song', state: 'searching' }), 'Asking around on Soulseek');
	assert.equal(describe({ ...album, state: 'waiting', note: '4th in their queue' }), '4th in their queue');
	assert.equal(describe({ ...album, state: 'downloading', progress: 1 }), 'Downloading · 100%');
});

test('finished and retryable', () => {
	assert.ok(finished({ state: 'done' }) && finished({ state: 'failed' }) && finished({ state: 'notFound' }));
	assert.ok(!finished({ state: 'importing' }));
	assert.ok(retryable({ state: 'notFound' }) && !retryable({ state: 'done' }));
});

test('latest: the whole album and each song apart', () => {
	const jobs = [
		{ id: 3, album: 'a', track: 7 },
		{ id: 2, album: 'a', track: null },
		{ id: 1, album: 'a', track: null }
	];
	assert.equal(latest(jobs, 'a')?.id, 2);
	assert.equal(latest(jobs, 'a', 7)?.id, 3);
	assert.equal(latest(jobs, 'a', 8), undefined);
	assert.equal(latest(jobs, 'b'), undefined);
});
