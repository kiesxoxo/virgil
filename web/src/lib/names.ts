// Handles for people calling in, and their call signs.

const ADJECTIVES = [
	'Velvet', 'Midnight', 'Static', 'Late', 'Neon', 'Quiet', 'Lunar', 'Golden', 'Electric', 'Silver',
	'Analog', 'Hazy', 'Slow', 'Cosmic', 'Paper', 'Night', 'Echo', 'Low', 'Wild', 'Lucky'
];
const NOUNS = [
	'Moth', 'Owl', 'Fox', 'Tuner', 'Signal', 'Comet', 'Heron', 'Lynx', 'Otter', 'Sparrow',
	'Raven', 'Dial', 'Wave', 'Moon', 'Hare', 'Crane', 'Koi', 'Bat', 'Lark', 'Wolf'
];

/** How many call-sign colours there are (--who-0 … --who-7). */
export const COLORS = 8;

/** A random handle: "Velvet Moth". */
export function handle(random: () => number = Math.random): string {
	const pick = (words: string[]) => words[Math.floor(random() * words.length) % words.length];
	return `${pick(ADJECTIVES)} ${pick(NOUNS)}`;
}

/**
 * A call sign for `name`: the initials of up to three words, or the first three letters of one
 * word. When someone has it already, its last letter gives way to a digit.
 */
export function signFor(name: string, taken: readonly string[] = []): string {
	const words = name
		.toUpperCase()
		.split(/\s+/)
		.map((w) => w.replace(/[^\p{L}\p{N}]/gu, ''))
		.filter(Boolean);
	// By letter, not by UTF-16 unit: half of a letter from beyond the basic plane is no letter.
	const letters = (words.length > 1 ? words.map((w) => [...w][0]) : [...(words[0] ?? '?')]).slice(0, 3);
	const sign = letters.join('');
	if (!taken.includes(sign)) return sign;
	const stem = letters.slice(0, 2).join('');
	for (let n = 2; n <= 9; n++) if (!taken.includes(`${stem}${n}`)) return `${stem}${n}`;
	return sign;
}

/** The first colour nobody has, or the least used. */
export function freeColor(taken: readonly number[]): number {
	const counts = Array.from({ length: COLORS }, (_, c) => taken.filter((t) => t === c).length);
	return counts.indexOf(Math.min(...counts));
}
