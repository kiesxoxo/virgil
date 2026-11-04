// Sizing a title in the wide cut: it steps down as it gets longer, and no single word overflows.

/** About how wide a character is in the wide cut, in em. */
function width(ch: string): number {
	if (ch > '⹿') return 1.02; // Japanese and the like, in the fallback font
	if (/\p{Lu}/u.test(ch)) return 0.95;
	if (/\p{Ll}/u.test(ch)) return 0.78;
	if (/\p{Nd}/u.test(ch)) return 0.93;
	return 0.55;
}

/** The widest stretch of `text` that can't break across lines, in em. */
export function longestRun(text: string): number {
	let longest = 0;
	let run = 0;
	for (const ch of text) {
		const wide = width(ch);
		// Spaces break a line; so does any character of a script written without them.
		run = /\s/.test(ch) || ch > '⹿' ? 0 : run + wide;
		longest = Math.max(longest, ch > '⹿' ? wide : run);
	}
	return longest;
}

/** Sizes in px, by length, for a title on a line as wide as a phone (`BASE` px). */
const STEPS: [length: number, size: number][] = [
	[8, 50],
	[14, 40],
	[24, 31],
	[44, 24]
];
const LONGEST = 20;
const BASE = 366;

/** The font size for `text` on lines `room` px wide, `scale` times the usual and at most `max`. */
export function fitTitle(text: string, room: number, { scale = 1, max = 88 }: { scale?: number; max?: number } = {}): number {
	const length = [...text].length;
	const step = STEPS.find(([most]) => length <= most)?.[1] ?? LONGEST;
	const size = Math.min(max, (step * scale * room) / BASE);
	const run = longestRun(text);
	return Math.floor(run > 0 ? Math.min(size, room / run) : size);
}
