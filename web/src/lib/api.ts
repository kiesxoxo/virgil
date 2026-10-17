import { toasts } from './toast.svelte.ts';
import type { AddRequest } from './types/AddRequest.ts';
import type { Album } from './types/Album.ts';
import type { AlbumDetail } from './types/AlbumDetail.ts';
import type { ModesRequest } from './types/ModesRequest.ts';
import type { SearchResults } from './types/SearchResults.ts';

async function call(method: string, path: string, body?: unknown, signal?: AbortSignal): Promise<Response> {
	let res: Response;
	try {
		res = await fetch(path, {
			method,
			signal,
			headers: body === undefined ? undefined : { 'content-type': 'application/json' },
			body: body === undefined ? undefined : JSON.stringify(body)
		});
	} catch (e) {
		if ((e as Error).name !== 'AbortError') toasts.show('hifi is not reachable');
		throw e;
	}
	if (!res.ok) {
		let message = res.statusText;
		try {
			message = (await res.json()).error ?? message;
		} catch {
			/* not JSON */
		}
		if (res.status !== 404) toasts.show(message);
		throw new ApiError(res.status, message);
	}
	return res;
}

export class ApiError extends Error {
	constructor(
		readonly status: number,
		message: string
	) {
		super(message);
	}
}

const post = (path: string, body?: unknown) => call('POST', path, body).then(() => undefined);
const get = <T>(path: string, signal?: AbortSignal) => call('GET', path, undefined, signal).then((r) => r.json() as Promise<T>);

export const api = {
	play: () => post('/api/player/play'),
	pause: () => post('/api/player/pause'),
	toggle: () => post('/api/player/toggle'),
	next: () => post('/api/player/next'),
	previous: () => post('/api/player/previous'),
	seek: (seconds: number) => post('/api/player/seek', { seconds }),
	modes: (modes: ModesRequest) => post('/api/player/modes', modes),

	add: (req: AddRequest) => post('/api/queue', req),
	clear: () => call('DELETE', '/api/queue').then(() => undefined),
	remove: (id: number) => call('DELETE', `/api/queue/${id}`).then(() => undefined),
	playItem: (id: number) => post(`/api/queue/${id}/play`),
	move: (id: number, to: number) => post(`/api/queue/${id}/move`, { to }),

	albums: () => get<Album[]>('/api/library/albums'),
	album: (id: string) => get<AlbumDetail>(`/api/library/albums/${encodeURIComponent(id)}`),
	search: (q: string, signal?: AbortSignal) =>
		get<SearchResults>(`/api/library/search?q=${encodeURIComponent(q)}`, signal),
	rescan: () => post('/api/library/rescan')
};

/** Queue helpers shared by album, search and track views. */
export const queue = {
	playAlbum: (album: string, start?: string) => api.add({ uris: [], album, start, mode: 'replace', play: true }),
	albumNext: (album: string) => api.add({ uris: [], album, mode: 'next', play: false }),
	albumLast: (album: string) => api.add({ uris: [], album, mode: 'append', play: false }),
	trackNext: (uri: string) => api.add({ uris: [uri], mode: 'next', play: false }),
	trackLast: (uri: string) => api.add({ uris: [uri], mode: 'append', play: false })
};
