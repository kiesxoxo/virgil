// node --test src/lib/coverflow.test.ts

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { TOP, backToFront, hitTest, layout, place, project, spin, visible } from './coverflow.ts';

const phone = layout(390, 640);
const close = (a: number, b: number) => assert.ok(Math.abs(a - b) < 1e-6, `${a} ≈ ${b}`);

test('layout: as big as fits, within bounds', () => {
	assert.equal(phone.cover, Math.round(390 * 0.68));
	assert.equal(layout(1600, 2000).cover, 340);
	assert.equal(layout(390, 200).cover, 120);
	assert.equal(phone.height, TOP + phone.cover + phone.reflection);
});

test('place: the middle cover lies flat where CSS put it', () => {
	const p = place(phone, 7, 7);
	assert.deepEqual([p.x, p.z, p.d], [195, -0, 0]);
	const h = phone.cover / 2;
	assert.deepEqual(project(phone, p, -h, -h), [195 - h, TOP]);
	assert.deepEqual(project(phone, p, h, h), [195 + h, TOP + phone.cover]);
});

test('place: neighbours mirror each other, turned and set back', () => {
	const left = place(phone, 6, 7);
	const right = place(phone, 8, 7);
	close(left.x - 195, 195 - right.x);
	close(left.z, right.z);
	close(left.angle, -right.angle);
	assert.ok(right.z < 0);
	// Turned towards the middle: the edge nearer the middle is further back, so smaller.
	const h = phone.cover / 2;
	const [, innerTop] = project(phone, right, -h, -h);
	const [, outerTop] = project(phone, right, h, -h);
	assert.ok(innerTop > outerTop);
});

test('place: half way between two covers, both turn half way', () => {
	const a = place(phone, 7, 7.5);
	const b = place(phone, 8, 7.5);
	close(a.d, -0.5);
	close(b.d, 0.5);
	close(a.x - 195, 195 - b.x);
});

test('visible: every cover outside the range is off screen', () => {
	for (const l of [phone, layout(1024, 700), layout(1920, 1000)]) {
		const { first, last } = visible(l, 50, 100);
		const h = l.cover / 2;
		const [right] = project(l, place(l, first - 1, 50), h, 0);
		const [left] = project(l, place(l, last, 50), -h, 0);
		assert.ok(right < 0, `cover ${first - 1} ends at ${right}`);
		assert.ok(left > l.width, `cover ${last} starts at ${left}`);
	}
	assert.deepEqual(visible(phone, 0, 3), { first: 0, last: 3 });
});

test('backToFront: the middle cover last', () => {
	const order = backToFront(3, 8, 5.2);
	assert.equal(order.at(-1), 5);
	assert.equal(order.at(-2), 6);
	assert.equal(order[0], 3);
});

test('hitTest: the cover in front wins, empty space misses', () => {
	const h = phone.cover / 2;
	assert.equal(hitTest(phone, 7, 100, 195, TOP + h), 7);
	assert.equal(hitTest(phone, 7, 100, 195, TOP - 2), -1);
	// Just right of the middle cover: the next one.
	const [x] = project(phone, place(phone, 8, 7), 0, 0);
	assert.equal(hitTest(phone, 7, 100, x, TOP + h), 8);
	// Where covers 8 and 9 overlap, 8 is in front.
	const [edge] = project(phone, place(phone, 9, 7), -h, 0);
	assert.equal(hitTest(phone, 7, 100, edge + 2, TOP + h), 8);
	// No cover past the end of the list.
	assert.equal(hitTest(phone, 99, 100, 390 - 2, TOP + h), -1);
});

test('project: turned with the phone, the middle cover turns about its centre', () => {
	const p = place(phone, 7, 7);
	const h = phone.cover / 2;
	const centre = TOP + h;
	assert.deepEqual(project(phone, p, h, h, [0, 0]), project(phone, p, h, h));
	close(project(phone, p, 0, 0, [1, -1])[0], 195);
	close(project(phone, p, 0, 0, [1, -1])[1], centre);
	// Tipped right, as CSS rotateY: the right edge goes back (smaller), the left comes forward.
	const [right, rightTop] = project(phone, p, h, -h, [1, 0]);
	const [left, leftTop] = project(phone, p, -h, -h, [1, 0]);
	assert.ok(right - 195 < 195 - left);
	assert.ok(rightTop > leftTop);
	// Tipped towards the viewer, as rotateX(-…): the bottom edge goes back.
	const [, bottom] = project(phone, p, 0, h, [0, 1]);
	const [, top] = project(phone, p, 0, -h, [0, 1]);
	assert.ok(bottom - centre < centre - top);
});

test('hitTest: taps land on the covers as they are turned', () => {
	const h = phone.cover / 2;
	const turn: [number, number] = [1, 0.5];
	assert.equal(hitTest(phone, 7, 100, 195, TOP + h, turn), 7);
	const [x, y] = project(phone, place(phone, 8, 7), 0, 0, turn);
	assert.equal(hitTest(phone, 7, 100, x, y, turn), 8);
	// The middle cover's right edge has moved in: just inside where it was flat is no longer it.
	assert.equal(hitTest(phone, 7, 100, 195 + h - 1, TOP + h), 7);
	assert.notEqual(hitTest(phone, 7, 100, 195 + h - 1, TOP + h, turn), 7);
});

test('spin: from the cover in the middle to the pick, never past either end', () => {
	for (const [from, to, count] of [
		[0, 900, 1000],
		[999, 3, 1000],
		[500, 510, 1000],
		[2, 5, 8],
		[7, 0, 8],
		[0, 1, 2],
		[40, 20, 45]
	]) {
		const s = spin(from, to, count);
		assert.equal(s.at(0), from);
		assert.equal(s.at(s.duration), to);
		for (let t = 0; t <= s.duration; t += 1 / 120) {
			const p = s.at(t);
			assert.ok(p >= 0 && p <= count - 1, `${from}→${to} of ${count}: ${p} at ${t}`);
		}
		// It ends slowly: the last 1/30 s moves less than a tenth of a cover.
		assert.ok(Math.abs(s.at(s.duration - 1 / 30) - to) < 0.1);
	}
});
