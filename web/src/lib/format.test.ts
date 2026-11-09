// node --test src/lib/format.test.ts

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { plural } from './format.ts';

test('plural: a count and its word', () => {
	assert.equal(plural(1, 'song'), '1 song');
	assert.equal(plural(0, 'song'), '0 songs');
	assert.equal(plural(2, 'album'), '2 albums');
});
