// Menu items for albums, tracks and queue entries, shared by every view.

import { ArrowUpToLine, Disc3, ListEnd, ListPlus, ListX, MessageSquareHeart, Phone, Play, UserRound } from '@lucide/svelte';
import { goto } from '$app/navigation';
import { page } from '$app/state';
import { api } from './api.ts';
import type { MenuHeader, MenuItem } from './menu.svelte.ts';
import { line } from './party.svelte.ts';
import { queue } from './request.svelte.ts';
import { radio } from './radio.svelte.ts';
import type { Album } from './types/Album.ts';
import type { QueueItem } from './types/QueueItem.ts';
import type { Track } from './types/Track.ts';

/** The item that goes to `href`, unless that's the page the menu is on. */
const goTo = (label: string, icon: MenuItem['icon'], href: string): MenuItem[] =>
	page.url.pathname === href ? [] : [{ label, icon, run: () => goto(href) }];

/**
 * With the request line open: Request, Request with a dedication, and for hosts Play now and Play
 * next. `what` is an album or some of its songs.
 */
function requestItems(what: { album: string } | { uris: string[] }, title: string, subtitle: string): MenuItem[] {
	const items: MenuItem[] = [
		{ label: 'Request', icon: Phone, run: () => queue.request(what) },
		{
			label: 'Request with a dedication…',
			icon: MessageSquareHeart,
			run: () => line.dedicate({ title, subtitle, send: (note) => queue.request(what, note) })
		}
	];
	if (line.host) {
		items.push({ label: 'Play now', icon: Play, run: () => queue.playNow(what) });
		items.push({
			label: 'Play next',
			icon: ListPlus,
			run: () => ('album' in what ? queue.albumNext(what.album) : queue.tracksNext(what.uris))
		});
	}
	return items;
}

export function albumItems(album: Album): MenuItem[] {
	const artist = goTo('Go to artist', UserRound, `/artist/${encodeURIComponent(album.artist)}`);
	if (line.open) return [...requestItems({ album: album.id }, album.title, album.artist), ...artist];
	return [
		{ label: 'Play', icon: Play, run: () => queue.playAlbum(album.id) },
		{ label: 'Play next', icon: ListPlus, run: () => queue.albumNext(album.id) },
		{ label: 'Add to end of queue', icon: ListEnd, run: () => queue.albumLast(album.id) },
		...artist
	];
}

export const albumHeader = (album: Album): MenuHeader => ({ title: album.title, subtitle: album.artist, art: album.art });

/** For a track on an album: "Play" starts the album from that track. */
export function trackItems(track: Track, albumId: string): MenuItem[] {
	const album = goTo('Go to album', Disc3, `/album/${albumId}`);
	if (line.open) return [...requestItems({ uris: [track.uri] }, track.title, track.artist), ...album];
	return [
		{ label: 'Play', icon: Play, run: () => queue.playAlbum(albumId, track.uri) },
		{ label: 'Play next', icon: ListPlus, run: () => queue.tracksNext([track.uri]) },
		{ label: 'Add to end of queue', icon: ListEnd, run: () => queue.tracksLast([track.uri]) },
		...album
	];
}

export const trackHeader = (track: Track, art?: string | null): MenuHeader => ({
	title: track.title,
	subtitle: track.artist,
	art
});

/**
 * For a queue entry. `index` is its position and `current` the playing one's
 * (-1 when nothing plays); "Play next" moves it to right after the current song.
 */
export function queueItems(item: QueueItem, index: number, current: number): MenuItem[] {
	if (!line.host) {
		// Guests: where it's from, and taking back their own.
		const items = item.albumId ? goTo('Go to album', Disc3, `/album/${item.albumId}`) : [];
		if (line.mine(item)) items.push({ label: 'Remove from queue', icon: ListX, run: () => api.remove(item.id), danger: true });
		return items;
	}
	const items: MenuItem[] = [{ label: 'Play', icon: Play, run: () => api.playItem(item.id) }];
	if (current >= 0 && index !== current && index !== current + 1) {
		items.push({
			label: 'Play next',
			icon: ArrowUpToLine,
			run: () => api.move(item.id, index > current ? current + 1 : current)
		});
	}
	if (item.albumId) items.push(...goTo('Go to album', Disc3, `/album/${item.albumId}`));
	items.push({ label: 'Remove from queue', icon: ListX, run: () => api.remove(item.id), danger: true });
	return items;
}

/** For a request of several songs (an album) in the queue. */
export function requestBlockItems(items: QueueItem[], current: number, indexOf: (item: QueueItem) => number): MenuItem[] {
	const first = items[0];
	const request = first.pick?.request;
	const out: MenuItem[] = [];
	if (line.host && current >= 0 && indexOf(first) > current + 1) {
		// Moved as a whole, so the album stays together.
		out.push({
			label: 'Play next',
			icon: ArrowUpToLine,
			run: async () => {
				for (const [i, item] of items.entries()) await api.move(item.id, current + 1 + i);
			}
		});
	}
	if (first.albumId) out.push(...goTo('Go to album', Disc3, `/album/${first.albumId}`));
	if (request != null && (line.host || line.mine(first))) {
		out.push({ label: 'Remove request', icon: ListX, run: () => api.removeRequest(request), danger: true });
	} else if (line.host) {
		out.push({
			label: 'Remove from queue',
			icon: ListX,
			run: async () => {
				for (const item of items) await api.remove(item.id);
			},
			danger: true
		});
	}
	return out;
}

export function queueHeader(item: QueueItem): MenuHeader {
	const shown = radio.show(item);
	return { title: shown.title, subtitle: shown.subtitle, art: shown.art, src: shown.src };
}
