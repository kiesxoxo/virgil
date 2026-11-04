import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { countLines, fitTitle, longestRun } from './fit.ts';

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

	it('steps down until its lines fit the height', () => {
		const title = 'ロストエデンへのパス';
		const size = fitTitle(title, 378, { tall: 66 });
		assert.ok(size < fitTitle(title, 378));
		assert.ok(countLines(title, 378 / size) * size * 1.03 <= 66);
	});

	it('leaves a title alone when it fits the height', () => {
		assert.equal(fitTitle('Sonno', 366, { tall: 200 }), fitTitle('Sonno', 366));
	});

	it('steps down until it takes no more than so many lines', () => {
		const title = 'The Shape of Punk to Come: A Chimerical Bombination in 12 Bursts';
		const size = fitTitle(title, 378, { lines: 3 });
		assert.ok(countLines(title, 378 / size) <= 3);
		assert.equal(fitTitle('Sonno', 378, { lines: 3 }), fitTitle('Sonno', 378));
	});

	it('stops at the smallest size that reads', () => {
		assert.equal(fitTitle('The Shape of Punk to Come: A Chimerical Bombination in 12 Bursts', 366, { tall: 10 }), 16);
	});
});

describe('countLines', () => {
	it('keeps words whole', () => {
		assert.equal(countLines('aaa bbb', 3), 2);
		assert.equal(countLines('aaa bbb', 10), 1);
	});

	it('breaks Japanese anywhere', () => {
		assert.equal(countLines('新しい日の誕生', 3.1), 3);
	});
});
