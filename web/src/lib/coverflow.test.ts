// node --test src/lib/coverflow.test.ts

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { TOP, backToFront, hitTest, layout, place, poses, project, shuffle, visible, type Pose } from './coverflow.ts';

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

const desk = layout(1440, 900);
const rolls: [number, number, number][] = [
	[0, 900, 1000],
	[999, 3, 1000],
	[500, 502, 1000],
	[500, 497, 1000],
	[2, 5, 8],
	[7, 0, 8],
	[0, 1, 2],
	[40, 20, 45]
];

/** The same numbers every run. */
function seeded(seed: number) {
	return () => ((seed = (seed * 1103515245 + 12345) % 2147483648) / 2147483648);
}

const same = (a: Pose, b: Pose) => {
	for (const key of ['x', 'y', 'z', 'angle', 'roll', 'open'] as const) close(a[key], b[key]);
};

test('shuffle: starts as the flow at the middle cover and ends as the flow at the pick', () => {
	for (const l of [phone, desk]) {
		for (const [from, to, count] of rolls) {
			const s = shuffle(l, from, to, count, seeded(from + to));
			const start = s.at(0);
			for (const p of poses(l, from, count)) same(start.find((q) => q.index === p.index)!, p);
			// The pick out of sight is there too, under another cover.
			const pick = start.find((p) => p.index === to)!;
			if (!poses(l, from, count).some((p) => p.index === to)) {
				const over = start.find((p) => p !== pick && p.x === pick.x)!;
				assert.ok(start.indexOf(over) > start.indexOf(pick));
			}
			const end = s.at(s.duration);
			assert.deepEqual(end.map((p) => p.index), poses(l, to, count).map((p) => p.index));
			end.forEach((p, i) => same(p, poses(l, to, count)[i]));
		}
	}
});

test('shuffle: only the covers in sight at the start are shuffled, and those around the pick dealt', () => {
	for (const l of [phone, desk]) {
		for (const [from, to, count] of rolls) {
			const s = shuffle(l, from, to, count, seeded(from * to));
			const before = new Set([...poses(l, from, count).map((p) => p.index), to]);
			const after = new Set(poses(l, to, count).map((p) => p.index));
			for (let t = 0; t <= s.duration + 0.1; t += 1 / 60) {
				const drawn = s.at(t).map((p) => p.index);
				assert.equal(new Set(drawn).size, drawn.length, `a cover twice at ${t}`);
				for (const index of drawn) assert.ok((t < s.dealt ? before : after).has(index), `${from}→${to}: cover ${index} at ${t}`);
			}
		}
	}
});

test('shuffle: the pick lands last, flat on top in the middle', () => {
	for (const [from, to, count] of rolls) {
		const s = shuffle(phone, from, to, count, seeded(to));
		const top = s.at(s.dealt - 1e-9).at(-1)!;
		assert.equal(top.index, to);
		close(top.x, 195);
		assert.ok(Math.abs(top.z) < 1e-3 && Math.abs(top.roll) < 1e-3 && Math.abs(top.angle) < 1e-3);
		assert.ok(s.beats.every((beat) => beat < s.dealt));
		assert.equal(s.at(s.dealt).at(-1)!.index, to);
	}
});

test('shuffle: the piles take turns, each from the top, the pick last', () => {
	const s = shuffle(phone, 500, 900, 1000, () => 0.5);
	// Each card thrown is drawn over those before it.
	const order: number[] = [];
	for (let t = 0; t < s.dealt; t += 1 / 240) {
		const top = s.at(t).at(-1)!.index;
		if (top !== 500 && top !== order.at(-1)) order.push(top);
	}
	assert.equal(order.length, 11);
	assert.equal(order.at(-1), 900);
	const sides = order.slice(0, -1).map((index) => Math.sign(index - 500));
	sides.forEach((side, i) => i && assert.notEqual(side, sides[i - 1]));
	// Each pile from the top: nearest the middle first.
	for (const side of [-1, 1]) {
		const pile = order.filter((index) => index !== 900 && Math.sign(index - 500) === side);
		assert.deepEqual(pile, [1, 2, 3, 4, 5].map((k) => 500 + side * k));
	}
});

test('shuffle: the piles stay clear of the deck, so a card thrown from one is never behind it', () => {
	for (const l of [phone, desk]) {
		const s = shuffle(l, 500, 900, 1000, seeded(3));
		const h = l.cover / 2;
		const across = (p: Pose) =>
			([[-h, -h], [h, -h], [h, h], [-h, h]] as const).map(([u, v]) => project(l, { ...p, d: 0 }, u * Math.cos(p.roll) - v * Math.sin(p.roll), u * Math.sin(p.roll) + v * Math.cos(p.roll) + p.y)[0]);
		const turned = (55 * Math.PI) / 180;
		for (let t = 0.26; t < s.dealt; t += 1 / 120) {
			const poses = s.at(t);
			// In a pile, turned all the way; on the deck, flat (in the air, between).
			const deck = poses.filter((p) => p.angle === 0).flatMap(across);
			const [left, right] = [Math.min(...deck), Math.max(...deck)];
			for (const p of poses.filter((p) => Math.abs(Math.abs(p.angle) - turned) < 1e-9)) {
				const edges = across(p);
				assert.ok(p.angle > 0 ? Math.max(...edges) <= left : Math.min(...edges) >= right, `cover ${p.index} over the deck at ${t}`);
			}
		}
	}
});

test('shuffle: covers askew stay on the canvas and above the floor', () => {
	for (const l of [phone, desk, layout(390, 300)]) {
		const s = shuffle(l, 500, 900, 1000, seeded(7));
		const h = l.cover / 2;
		for (let t = 0; t <= s.duration; t += 1 / 120) {
			for (const p of s.at(t)) {
				const corners = ([[-h, -h], [h, -h], [h, h], [-h, h]] as const).map(([u, v]) =>
					project(l, { ...p, d: 0 }, u * Math.cos(p.roll) - v * Math.sin(p.roll), u * Math.sin(p.roll) + v * Math.cos(p.roll) + p.y)
				);
				const top = Math.min(...corners.map(([, y]) => y));
				assert.ok(top >= 0, `top at ${top} (${t})`);
				// Standing on its lowest corner, or lifted.
				const low = Math.max(...[-h, h].map((u) => u * Math.sin(Math.abs(p.roll)) + h * Math.cos(p.roll))) + p.y;
				assert.ok(low <= h + 1e-6, `${low} below the floor (${t})`);
			}
		}
	}
});
