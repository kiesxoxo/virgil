import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { fitTitle, longestRun } from './fit.ts';

describe('longestRun', () => {
	it('is the widest word', () => {
		assert.ok(Math.abs(longestRun('a bb ccc') - 0.78 * 3) < 1e-9);
	});

	it('counts capitals as wider', () => {
		assert.ok(longestRun('MASSIMO') > longestRun('massimo'));
	});

	it('treats Japanese as breaking anywhere', () => {
		assert.equal(longestRun('新しい日の誕生'), 1.02);
	});

	it('is nothing for nothing', () => {
		assert.equal(longestRun(''), 0);
	});
});

describe('fitTitle', () => {
	it('steps down with length', () => {
		assert.ok(fitTitle('Sonno', 366) > fitTitle('Volume Massimo', 366));
		assert.ok(fitTitle('Volume Massimo', 366) > fitTitle('Comafields / Imaginary Festival', 366));
	});

	it('holds a long title', () => {
		assert.equal(fitTitle('The Shape of Punk to Come: A Chimerical Bombination in 12 Bursts', 366), 20);
	});

	it('never lets a long word overflow', () => {
		const word = 'Destextrinum';
		const size = fitTitle(word, 366);
		assert.ok(size * longestRun(word) <= 366);
	});

	it('follows the room and stops at the most', () => {
		assert.equal(fitTitle('Sonno', 183), 25);
		assert.equal(fitTitle('Sonno', 2000, { max: 60 }), 60);
	});
});
