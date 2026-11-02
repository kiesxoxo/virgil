// node --test src/lib/virtual.test.ts

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { revealScroll, rowWindow, topRow } from './virtual.ts';

test('topRow: in whole rows, below the screen as negative', () => {
	assert.equal(topRow(0, 100), 0);
	assert.equal(topRow(99, 100), 0);
	assert.equal(topRow(100, 100), 1);
	assert.equal(topRow(-150, 100), -2);
	assert.equal(topRow(500, 0), 0);
});

test('rowWindow: the rows in sight and overscan each way, within the list', () => {
	// Rows 10–17 in sight (800 px of 100 px rows), 3 more each way.
	assert.deepEqual(rowWindow(10, 800, 100, 50, 3), { first: 7, last: 21 });
	// At the top, and the list below the top of the screen.
	assert.deepEqual(rowWindow(0, 800, 100, 50, 3), { first: 0, last: 11 });
	assert.deepEqual(rowWindow(-4, 800, 100, 50, 3), { first: 0, last: 7 });
	// At the end.
	assert.deepEqual(rowWindow(48, 800, 100, 50, 3), { first: 45, last: 50 });
	// Far below the screen: nothing.
	assert.deepEqual(rowWindow(-20, 800, 100, 50, 3), { first: 0, last: 0 });
	// Not measured yet.
	assert.deepEqual(rowWindow(0, 800, 0, 50, 3), { first: 0, last: 0 });
});

test('revealScroll: the item in the middle of the screen, never above the page', () => {
	// The list 200 px down; row 10's 160 px picture in the middle of an 800 px screen.
	assert.equal(revealScroll(200, 10, 230, 160, 800), 200 + 2300 + 80 - 400);
	assert.equal(revealScroll(200, 0, 230, 160, 800), 0);
});
