// node --test src/lib/palette.test.ts

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { hexRgb, oklab, palette, saturate, type Rgb } from './palette.ts';

/** An image made of `count` pixels of each colour. */
function image(...parts: [Rgb, number][]): Uint8ClampedArray {
	const px = parts.flatMap(([rgb, count]) => Array.from({ length: count }, () => [...rgb, 255]));
	return new Uint8ClampedArray(px.flat());
}

test('palette: colourful beats grey, then by size', () => {
	const colors = palette(image([[200, 40, 40], 100], [[128, 128, 128], 200], [[30, 60, 200], 150]), 3);
	assert.deepEqual(colors, [
		[30, 60, 200],
		[200, 40, 40],
		[128, 128, 128]
	]);
});

test('palette: near-black and near-white only when there is nothing else', () => {
	assert.deepEqual(palette(image([[0, 0, 0], 500], [[255, 255, 255], 400], [[40, 120, 60], 10]), 1), [[40, 120, 60]]);
	assert.deepEqual(palette(image([[0, 0, 0], 500], [[255, 255, 255], 400]), 1), [[0, 0, 0]]);
});

test('palette: similar colours count once', () => {
	const colors = palette(image([[200, 40, 40], 100], [[210, 45, 40], 90], [[30, 60, 200], 50]), 2);
	assert.deepEqual(colors, [
		[200, 40, 40],
		[30, 60, 200]
	]);
});

test('palette: one colour pads with shades of it', () => {
	const colors = palette(image([[100, 50, 150], 64]), 4);
	assert.equal(colors.length, 4);
	assert.deepEqual(colors[0], [100, 50, 150]);
	assert.equal(new Set(colors.map(String)).size, 4);
});

test('palette: transparent pixels are ignored', () => {
	const rgba = image([[200, 40, 40], 10]);
	const clear = new Uint8ClampedArray([30, 60, 200, 0, 30, 60, 200, 0]);
	assert.deepEqual(palette(new Uint8ClampedArray([...rgba, ...clear]), 1), [[200, 40, 40]]);
});

test('saturate', () => {
	assert.deepEqual(saturate([128, 128, 128], 1.5), [128, 128, 128]);
	assert.deepEqual(saturate([200, 100, 100], 1), [200, 100, 100]);
	const [r, g, b] = saturate([200, 100, 100], 1.5);
	assert.ok(r > 200 && g < 100 && b < 100);
	assert.deepEqual(saturate([250, 10, 10], 3), [255, 0, 0]);
});

test('oklab', () => {
	const near = (actual: number[], expected: number[]) => actual.forEach((v, i) => assert.ok(Math.abs(v - expected[i]) < 1e-3, `${actual} ≉ ${expected}`));
	near(oklab([255, 255, 255]), [1, 0, 0]);
	near(oklab([0, 0, 0]), [0, 0, 0]);
	near(oklab([255, 0, 0]), [0.628, 0.2249, 0.1258]);
});

test('hexRgb', () => {
	assert.deepEqual(hexRgb('#ec4a2f'), [236, 74, 47]);
	assert.deepEqual(hexRgb(' #fff'), [255, 255, 255]);
	assert.equal(hexRgb('rgb(1 2 3)'), null);
});
