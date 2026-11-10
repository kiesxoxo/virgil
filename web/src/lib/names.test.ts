import assert from 'node:assert/strict';
import { test } from 'node:test';
import { freeColor, handle, signFor } from './names.ts';

test('handles are two words', () => {
	assert.equal(handle(() => 0), 'Velvet Moth');
	assert.equal(handle(() => 0.999), 'Lucky Wolf');
});

test('call signs come from initials or the start of a name', () => {
	assert.equal(signFor('Velvet Moth'), 'VM');
	assert.equal(signFor('ola'), 'OLA');
	assert.equal(signFor('Marcin'), 'MAR');
	assert.equal(signFor('Jo'), 'JO');
	assert.equal(signFor('dj. late night owl'), 'DLN');
	assert.equal(signFor('Łucja'), 'ŁUC');
	assert.equal(signFor('  '), '?');
	// Letters that take two UTF-16 units each stay whole.
	assert.equal(signFor('𝔸𝔹𝔻𝔼'), '𝔸𝔹𝔻');
	assert.equal(signFor('𝔸𝔹𝔻𝔼', ['𝔸𝔹𝔻']), '𝔸𝔹2');
});

test('a taken call sign gets a digit', () => {
	assert.equal(signFor('Ola', ['OLA']), 'OL2');
	assert.equal(signFor('Velvet Moth', ['VM', 'VM2']), 'VM3');
});

test('colours go to whoever has none yet', () => {
	assert.equal(freeColor([]), 0);
	assert.equal(freeColor([0, 1, 3]), 2);
	assert.equal(freeColor([0, 1, 2, 3, 4, 5, 6, 7, 0]), 1);
});
