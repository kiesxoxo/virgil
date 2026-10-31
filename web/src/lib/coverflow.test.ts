// node --test src/lib/coverflow.test.ts

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { TOP, backToFront, hitTest, layout, place, project, visible } from './coverflow.ts';

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
