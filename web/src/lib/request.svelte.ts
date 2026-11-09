// Putting music in the queue, from every view. With the request line open nothing replaces the
// queue: playing means requesting, each request takes its turn, and a toast says where it landed.
// Hosts still have Play now and Play next.

import { api } from './api.ts';
import { span } from './format.ts';
import { line, ordinal } from './party.svelte.ts';
import { toasts } from './toast.svelte.ts';
import type { Added } from './types/Added.ts';
import type { AddRequest } from './types/AddRequest.ts';

type What = { album: string; start?: string } | { uris: string[] };

const body = (what: What): Pick<AddRequest, 'uris' | 'album' | 'start'> =>
	'album' in what ? { uris: [], album: what.album, start: what.start } : { uris: what.uris };

/** "You're next", "3rd in line · about 14 min". */
export function landed(added: Added): string {
	if (added.position === 0) return 'Playing now';
	if (added.position === 1) return line.host ? 'Playing next' : 'You’re next';
	const wait = added.ahead >= 60 ? ` · about ${span(added.ahead)}` : '';
	return `${ordinal(added.position)} in line${wait}`;
}

/** Requests `what` in its turn, with an optional dedication. */
async function request(what: What, note?: string) {
	const added = await api.add({ ...body(what), mode: 'append', play: false, note });
	toasts.done(landed(added));
	return added;
}

export const queue = {
	request,

	/** An album, or a song of it (`start`). With the line open: requests the album, or that one song. */
	playAlbum: (album: string, start?: string) =>
		line.open
			? request(start ? { uris: [start] } : { album })
			: api.add({ uris: [], album, start, mode: 'replace', play: true }),
	/** Hosts, with the line open: plays it right away and keeps the queue. */
	playNow: (what: What) => api.add({ ...body(what), mode: 'replace', play: true }),
	albumNext: (album: string) => next({ album }),
	albumLast: (album: string) => last({ album }),
	tracksNext: (uris: string[]) => next({ uris }),
	tracksLast: (uris: string[]) => last({ uris }),
	/** Several albums in order (an artist's, a genre's): the first replaces the queue, or each is a request. */
	playAlbums: async (albums: string[]) => {
		if (line.open) {
			let added: Added | null = null;
			for (const album of albums) {
				const one = await api.add({ uris: [], album, mode: 'append', play: false });
				added ??= one;
			}
			if (added) toasts.done(albums.length > 1 ? `${landed(added)} · ${albums.length} requests` : landed(added));
			return;
		}
		for (const [i, album] of albums.entries()) {
			await api.add({ uris: [], album, mode: i === 0 ? 'replace' : 'append', play: i === 0 });
		}
	},
	albumsLast: async (albums: string[]) => {
		if (line.open) return queue.playAlbums(albums);
		for (const album of albums) await api.add({ uris: [], album, mode: 'append', play: false });
		toasts.done('Added to the end of the queue');
	}
};

async function next(what: What) {
	if (line.open && !line.host) return request(what);
	const added = await api.add({ ...body(what), mode: 'next', play: false });
	toasts.done('Playing next');
	return added;
}

async function last(what: What) {
	if (line.open) return request(what);
	const added = await api.add({ ...body(what), mode: 'append', play: false });
	toasts.done('Added to the end of the queue');
	return added;
}
