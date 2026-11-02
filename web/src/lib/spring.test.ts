// node --test src/lib/spring.test.ts

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { advance, settled, spring } from './spring.ts';

/** Frames of `dt` until settled, or Infinity. */
function framesToSettle(s: ReturnType<typeof spring>, dt = 1 / 60, limit = 600) {
	for (let i = 1; i <= limit; i++) {
		advance(s, dt);
		if (settled(s)) return i;
	}
	return Infinity;
}

test('spring: gets there in about its response time, and stays', () => {
	const s = spring(0);
	s.target = 300;
	const frames = framesToSettle(s);
	assert.ok(frames > 10 && frames < 60, `${frames} frames`);
	advance(s, 1);
	assert.ok(Math.abs(s.value - 300) < 0.5);
});

test('spring: barely overshoots when nearly critically damped', () => {
	const s = spring(0);
	s.target = 100;
	let most = 0;
	for (let i = 0; i < 120; i++) {
		advance(s, 1 / 60);
		most = Math.max(most, s.value);
	}
	assert.ok(most < 101, `${most}`);
});

test('spring: a new target keeps the speed it had', () => {
	const s = spring(0);
	s.target = 100;
	for (let i = 0; i < 6; i++) advance(s, 1 / 60);
	const { value, velocity } = s;
	assert.ok(velocity > 0);
	s.target = -100;
	advance(s, 1 / 60);
	// Still going the old way for a moment, slower.
	assert.ok(s.value > value && s.velocity < velocity);
});

test('spring: frame times do not change where it goes', () => {
	const [a, b] = [spring(0), spring(0)];
	a.target = b.target = 50;
	for (let i = 0; i < 30; i++) advance(a, 1 / 60);
	for (let i = 0; i < 15; i++) advance(b, 1 / 30);
	assert.ok(Math.abs(a.value - b.value) < 0.01, `${a.value} ${b.value}`);
});

test('settled: not while moving through the target', () => {
	assert.ok(settled({ value: 10, velocity: 0, target: 10 }));
	assert.ok(!settled({ value: 10, velocity: 400, target: 10 }));
	assert.ok(!settled({ value: 12, velocity: 0, target: 10 }));
});
