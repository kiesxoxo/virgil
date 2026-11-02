// node --test src/lib/flyers.test.ts

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { layout, middle, place, project, turnAngles, type Turn } from './coverflow.ts';
import { apply, cardMatrix } from './flyers.ts';

const l = layout(390, 700);

/** A flow cover as a card, as flowCards in stage-gl.ts makes it (the flow at the screen's top left). */
function flowCard(index: number, position: number, turn: Turn) {
	const p = place(l, index, position);
	return {
		card: {
			eye: [l.width / 2, middle(l)] as [number, number],
			perspective: l.perspective,
			centre: [p.x - l.width / 2, 0, p.z] as [number, number, number],
			angle: p.angle,
			turn: turnAngles(turn),
			size: l.cover
		},
		p
	};
}

const near = (a: [number, number], b: [number, number]) =>
	assert.ok(Math.hypot(a[0] - b[0], a[1] - b[1]) < 0.5, `${a.map((n) => n.toFixed(2))} vs ${b.map((n) => n.toFixed(2))}`);

test('cardMatrix: a flow cover lands where the flow draws it, turned, set back and tilted', () => {
	const h = l.cover / 2;
	for (const [index, position, turn] of [
		[5, 5, [0, 0]],
		[6, 5, [0, 0]],
		[3, 5, [0.6, -0.4]],
		[5, 4.5, [-1, 1]]
	] as [number, number, Turn][]) {
		const { card, p } = flowCard(index, position, turn);
		for (const base of [l.cover, 500]) {
			const m = cardMatrix(card, base);
			for (const [u, v] of [[-h, -h], [h, -h], [h, h], [-h, h]]) {
				const k = base / l.cover;
				near(apply(m, (u + h) * k, (v + h) * k), project(l, p, u, v, turn));
			}
		}
	}
});

test('cardMatrix: a flat cover lies on its own pixels, whatever the camera', () => {
	const card = { eye: [100, 200] as [number, number], perspective: 900, centre: [40, -20, 0] as [number, number, number], angle: 0, turn: [0, 0] as [number, number], size: 120 };
	const m = cardMatrix(card, 300);
	near(apply(m, 0, 0), [80, 120]);
	near(apply(m, 300, 300), [200, 240]);
});
