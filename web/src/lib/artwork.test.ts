// node --test src/lib/artwork.test.ts

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { isDecoded, markDecoded, standIn } from './artwork.ts';

test('standIn: the sharpest size decoded, other than the one asked for', () => {
	assert.equal(standIn('a1'), null);
	markDecoded('/art/a1-300.jpg');
	assert.equal(standIn('a1'), '/art/a1-300.jpg');
	markDecoded('http://virgil.local/art/a1-1000.jpg');
	assert.equal(standIn('a1'), '/art/a1-1000.jpg');
	assert.equal(standIn('a1', '/art/a1-1000.jpg'), '/art/a1-300.jpg');
	assert.equal(standIn('a2'), null);
	assert.equal(standIn(null), null);
});

test('isDecoded: full URLs and paths alike', () => {
	markDecoded('https://virgil.home/art/b1-600.jpg');
	assert.ok(isDecoded('/art/b1-600.jpg'));
	assert.ok(isDecoded('https://elsewhere/art/b1-600.jpg'));
	assert.ok(!isDecoded('/art/b1-300.jpg'));
	assert.ok(!isDecoded(null));
});

test('markDecoded: the least recently decoded go first', () => {
	markDecoded('/art/keep-300.jpg');
	for (let i = 0; i < 299; i++) {
		markDecoded(`/art/c${i}-300.jpg`);
		// Used again: stays.
		if (i % 50 === 0) markDecoded('/art/keep-300.jpg');
	}
	assert.ok(isDecoded('/art/keep-300.jpg'));
	assert.ok(!isDecoded('/art/a1-300.jpg'));
	assert.ok(isDecoded('/art/c298-300.jpg'));
});
