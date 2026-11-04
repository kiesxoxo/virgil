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

/** About how many lines `text` takes, `room` em wide. */
export function countLines(text: string, room: number): number {
	let lines = 1;
	let line = 0;
	let word = 0;
	const flush = () => {
		if (line && line + word > room) {
			lines++;
			line = 0;
		}
		line += word;
		word = 0;
	};
	for (const ch of text) {
		if (/\s/.test(ch)) {
			flush();
			line += width(' ');
		} else {
			word += width(ch);
			if (ch > '⹿') flush();
		}
	}
	flush();
	return lines;
}

const LEADING = 1.03;
const SMALLEST = 16;

/**
 * The font size for `text` on lines `room` px wide, `scale` times the usual and at most `max`.
 * Given `tall`, the px it has going down, it steps down further until its lines fit in that;
 * given `lines`, until it takes no more than that many.
 */
export function fitTitle(
	text: string,
	room: number,
	{ scale = 1, max = 88, tall = 0, lines = 0 }: { scale?: number; max?: number; tall?: number; lines?: number } = {}
): number {
	const length = [...text].length;
	const step = STEPS.find(([most]) => length <= most)?.[1] ?? LONGEST;
	const wanted = Math.min(max, (step * scale * room) / BASE);
	const run = longestRun(text);
	let size = Math.floor(run > 0 ? Math.min(wanted, room / run) : wanted);
	const fits = () => {
		const count = countLines(text, room / size);
		return (!tall || tall < 0 || count * size * LEADING <= tall) && (!lines || count <= lines);
	};
	while (size > SMALLEST && !fits()) size--;
	return size;
}
