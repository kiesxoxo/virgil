import { toasts } from './toast.svelte.ts';
import type { AddRequest } from './types/AddRequest.ts';
import type { Album } from './types/Album.ts';
import type { ArtOutcome } from './types/ArtOutcome.ts';
import type { AlbumDetail } from './types/AlbumDetail.ts';
import type { LookupQuery } from './types/LookupQuery.ts';
import type { LookupRelease } from './types/LookupRelease.ts';
import type { LookupResults } from './types/LookupResults.ts';
import type { LookupSource } from './types/LookupSource.ts';
import type { ModesRequest } from './types/ModesRequest.ts';
import type { OnlineSettings } from './types/OnlineSettings.ts';
import type { OnlineSettingsUpdate } from './types/OnlineSettingsUpdate.ts';
import type { SearchResults } from './types/SearchResults.ts';
import type { TagEdit } from './types/TagEdit.ts';
import type { TaggedFile } from './types/TaggedFile.ts';
import type { TagWriteResult } from './types/TagWriteResult.ts';

async function call(method: string, path: string, body?: unknown, signal?: AbortSignal): Promise<Response> {
	let res: Response;
	try {
		// A Blob (e.g. a picked file) goes as the raw body; anything else as JSON.
		const raw = body instanceof Blob;
		res = await fetch(path, {
			method,
			signal,
			headers: body === undefined || raw ? undefined : { 'content-type': 'application/json' },
			body: body === undefined ? undefined : raw ? body : JSON.stringify(body)
		});
	} catch (e) {
		if ((e as Error).name !== 'AbortError') toasts.show('Virgil is not reachable');
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
	rescan: () => post('/api/library/rescan'),
	downloadArt: (id: string) =>
		call('POST', `/api/library/albums/${encodeURIComponent(id)}/art`).then((r) => r.json() as Promise<ArtOutcome>),
	downloadMissingArt: () => post('/api/library/art/download-missing'),
	uploadArt: (id: string, image: Blob) => call('POST', `/api/library/albums/${encodeURIComponent(id)}/art/upload`, image),

	/** Releases that might be the album, from MusicBrainz and Discogs. */
	lookupSearch: (query: LookupQuery) => call('POST', '/api/lookup/search', query).then((r) => r.json() as Promise<LookupResults>),
	lookupRelease: (source: LookupSource, id: string) =>
		get<LookupRelease>(`/api/lookup/${source}/${encodeURIComponent(id)}`),
	onlineSettings: () => get<OnlineSettings>('/api/settings/online'),
	saveOnlineSettings: (update: OnlineSettingsUpdate) =>
		call('POST', '/api/settings/online', update).then((r) => r.json() as Promise<OnlineSettings>),

	/** Plays an NTS station right after the current song (the queue carries on after it). */
	playStation: (id: string) => post(`/api/radio/${encodeURIComponent(id)}/play`),

	selectOutput: (id: number) => post(`/api/outputs/${id}/select`),
	restart: () => post('/api/restart'),

	readTags: (uris: string[]) => call('POST', '/api/tags/read', { uris }).then((r) => r.json() as Promise<TaggedFile[]>),
	writeTags: (edits: TagEdit[]) =>
		call('POST', '/api/tags/write', { edits }).then((r) => r.json() as Promise<TagWriteResult>)
};

const next = () => toasts.done('Playing next');
const last = () => toasts.done('Added to the end of the queue');

/** Queue helpers shared by album, search and track views. */
export const queue = {
	playAlbum: (album: string, start?: string) => api.add({ uris: [], album, start, mode: 'replace', play: true }),
	albumNext: (album: string) => api.add({ uris: [], album, mode: 'next', play: false }).then(next),
	albumLast: (album: string) => api.add({ uris: [], album, mode: 'append', play: false }).then(last),
	trackNext: (uri: string) => api.add({ uris: [uri], mode: 'next', play: false }).then(next),
	trackLast: (uri: string) => api.add({ uris: [uri], mode: 'append', play: false }).then(last),
	/** Plays several albums in order (an artist's, a genre's): the first replaces the queue. */
	playAlbums: async (albums: string[]) => {
		for (const [i, album] of albums.entries()) {
			await api.add({ uris: [], album, mode: i === 0 ? 'replace' : 'append', play: i === 0 });
		}
	},
	albumsLast: async (albums: string[]) => {
		for (const album of albums) await api.add({ uris: [], album, mode: 'append', play: false });
		last();
	}
};
