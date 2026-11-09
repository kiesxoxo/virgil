// The queue as requests: the songs of one request together (an album stays whole), and songs
// nobody requested grouped by album. virgild takes turns by the same groups (crates/virgild/src/turns.rs).

import type { Pick } from './types/Pick.ts';
import type { QueueItem } from './types/QueueItem.ts';

export interface Block {
	key: string;
	items: QueueItem[];
	pick: Pick | null;
	/** Two or more songs from one album: shown as the album. */
	album: boolean;
}

const keyOf = (item: QueueItem) =>
	item.pick ? `r${item.pick.request}` : item.albumId ? `a${item.albumId}` : `s${item.id}`;

export function blocks(items: readonly QueueItem[]): Block[] {
	const out: Block[] = [];
	for (const item of items) {
		const key = keyOf(item);
		const last = out.at(-1);
		if (last && keyOf(last.items[0]) === key) last.items.push(item);
		else out.push({ key: `${key}:${item.id}`, items: [item], pick: item.pick, album: false });
	}
	for (const block of out) {
		const album = block.items[0].albumId;
		block.album = block.items.length > 1 && album != null && block.items.every((i) => i.albumId === album);
	}
	return out;
}
