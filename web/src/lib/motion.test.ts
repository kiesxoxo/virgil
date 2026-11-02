// node --test src/lib/motion.test.ts

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { filter, screenAxes, step } from './motion.ts';

/** Readings every 16 ms for `ms`, returning the last value passed on and how many were. */
function hold(f: ReturnType<typeof filter>, beta: number, gamma: number, ms: number, angle = 0) {
	let last: [number, number] | null = null;
	let sent = 0;
	for (let t = 0; t < ms; t += 16) {
		const v = step(f, beta, gamma, angle, 16);
		if (v) [last, sent] = [v, sent + 1];
	}
	return { last, sent };
}

test('step: however the phone is held, held still is level', () => {
	const f = filter();
	assert.equal(step(f, 70, -20, 0, 0), null);
	hold(f, 70, -20, 2000);
	assert.deepEqual(f.sent, [0, 0]);
});

test('step: a quick turn tilts, then settles back and goes quiet', () => {
	const f = filter();
	hold(f, 45, 0, 1000);
	const { last } = hold(f, 45, 10, 300);
	assert.ok(last && last[0] > 0.2 && Math.abs(last[1]) < 1e-9, `${last}`);
	const settled = hold(f, 45, 10, 8000);
	assert.deepEqual(settled.last, [0, 0]);
	assert.equal(hold(f, 45, 10, 2000).sent, 0);
});

test('step: never past full tilt', () => {
	const f = filter();
	hold(f, 0, 0, 500);
	hold(f, 40, -40, 500);
	assert.ok(f.value.every((v) => Math.abs(v) <= 1));
});

test('step: the sensor flipping over starts again rather than jumping', () => {
	const f = filter();
	hold(f, 80, 30, 1000);
	step(f, 100, -30, 0, 16);
	assert.ok(Math.abs(f.value[0]) < 0.2);
});

test('screenAxes: landscape swaps the axes', () => {
	assert.deepEqual(screenAxes(10, 20, 0), [20, 10]);
	assert.deepEqual(screenAxes(10, 20, 90), [10, -20]);
	assert.deepEqual(screenAxes(10, 20, -90), [-10, 20]);
	assert.deepEqual(screenAxes(10, 20, 180), [-20, -10]);
});
