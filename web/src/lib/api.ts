import { toasts } from './toast.svelte.ts';
import { token } from './token.ts';
import type { Added } from './types/Added.ts';
import type { AddRequest } from './types/AddRequest.ts';
import type { Aircheck } from './types/Aircheck.ts';
import type { Album } from './types/Album.ts';
import type { AlbumDetail } from './types/AlbumDetail.ts';
import type { ModesRequest } from './types/ModesRequest.ts';
import type { PartyRequest } from './types/PartyRequest.ts';
import type { Person } from './types/Person.ts';
import type { ProfileRequest } from './types/ProfileRequest.ts';
import type { SearchResults } from './types/SearchResults.ts';

async function call(method: string, path: string, body?: unknown, signal?: AbortSignal): Promise<Response> {
	let res: Response;
	try {
		// A Blob (e.g. a picked file) goes as the raw body; anything else as JSON.
		const raw = body instanceof Blob;
		// Every request says which device it comes from: virgild knows who that is.
		const headers: Record<string, string> = { 'x-virgil-token': token };
		if (body !== undefined && !raw) headers['content-type'] = 'application/json';
		res = await fetch(path, {
			method,
			signal,
			headers,
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
const json = <T>(method: string, path: string, body?: unknown) => call(method, path, body).then((r) => r.json() as Promise<T>);

export const api = {
	play: () => post('/api/player/play'),
	toggle: () => post('/api/player/toggle'),
	next: () => post('/api/player/next'),
	previous: () => post('/api/player/previous'),
	seek: (seconds: number) => post('/api/player/seek', { seconds }),
	modes: (modes: ModesRequest) => post('/api/player/modes', modes),

	add: (req: AddRequest) => json<Added>('POST', '/api/queue', req),
	clear: () => call('DELETE', '/api/queue').then(() => undefined),
	remove: (id: number) => call('DELETE', `/api/queue/${id}`).then(() => undefined),
	/** Removes what is left of a request; when it is playing, that skips it. */
	removeRequest: (request: number) => call('DELETE', `/api/queue/requests/${request}`).then(() => undefined),
	playItem: (id: number) => post(`/api/queue/${id}/play`),
	move: (id: number, to: number) => post(`/api/queue/${id}/move`, { to }),

	albums: () => get<Album[]>('/api/library/albums'),
	album: (id: string) => get<AlbumDetail>(`/api/library/albums/${encodeURIComponent(id)}`),
	search: (q: string, signal?: AbortSignal) =>
		get<SearchResults>(`/api/library/search?q=${encodeURIComponent(q)}`, signal),
	rescan: () => post('/api/library/rescan'),

	/** Plays an NTS station right after the current song (the queue carries on after it). */
	playStation: (id: string) => post(`/api/radio/${encodeURIComponent(id)}/play`),

	/** Who this device is; a 404 (no toast) until it has said. */
	me: () => get<Person>('/api/people/me'),
	setMe: (profile: ProfileRequest) => json<Person>('PUT', '/api/people/me', profile),
	linkMe: (person: string) => json<Person>('POST', '/api/people/me/link', { person }),
	setHost: (id: string, host: boolean) => post(`/api/people/${id}/host`, { host }),
	forget: (id: string) => call('DELETE', `/api/people/${id}`).then(() => undefined),
	party: (req: PartyRequest) => post('/api/party', req),
	/** Skips the song playing, or votes to, depending on who asks and the rules. */
	skip: () => post('/api/party/skip'),
	cheer: () => post('/api/party/cheer'),
	aircheck: () => get<Aircheck>('/api/party/aircheck'),

	selectOutput: (id: number) => post(`/api/outputs/${id}/select`),
	restart: () => post('/api/restart'),
	resetHomekit: () => post('/api/homekit/reset')
};
