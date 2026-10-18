// Menu items for albums, tracks and queue entries, shared by every view.

import { ArrowUpToLine, ImageDown, ImageUp, ListEnd, ListPlus, ListX, Play, Tags } from '@lucide/svelte';
import { api, queue } from './api.ts';
import type { MenuHeader, MenuItem } from './menu.svelte.ts';
import { tagEditor } from './tageditor.svelte.ts';
import { toasts } from './toast.svelte.ts';
import type { Album } from './types/Album.ts';
import type { ArtOutcome } from './types/ArtOutcome.ts';
import type { QueueItem } from './types/QueueItem.ts';
import type { Track } from './types/Track.ts';

const artMessages: Record<ArtOutcome, string> = {
	saved: 'Artwork saved to the album folder',
	alreadyPresent: 'This album already has artwork',
	notFound: 'No matching artwork found online'
};

/** Covers bigger than this are refused by virgild. */
const MAX_UPLOAD = 20 << 20;

/**
 * Opens the file picker (must run straight from the tap, or iOS ignores it)
 * and saves the picked image as the album's cover.
 */
function uploadArt(album: Album) {
	const input = document.createElement('input');
	input.type = 'file';
	input.accept = 'image/jpeg,image/png,image/webp';
	input.hidden = true;
	const done = () => input.remove();
	input.addEventListener('cancel', done);
	input.addEventListener('change', () => {
		const file = input.files?.[0];
		done();
		if (!file) return;
		if (file.size > MAX_UPLOAD) return toasts.show('That image is too large (20 MB at most)');
		toasts.done('Uploading artwork…');
		api.uploadArt(album.id, file).then(
			() => toasts.done('Artwork saved to the album folder'),
			() => {} // the error toast is already up
		);
	});
	document.body.append(input);
	input.click();
}

export function albumItems(album: Album): MenuItem[] {
	const items: MenuItem[] = [
		{ label: 'Play', icon: Play, run: () => queue.playAlbum(album.id) },
		{ label: 'Play next', icon: ListPlus, run: () => queue.albumNext(album.id) },
		{ label: 'Add to end of queue', icon: ListEnd, run: () => queue.albumLast(album.id) }
	];
	if (!album.art) {
		items.push({
			label: 'Download artwork',
			icon: ImageDown,
			run: () => {
				toasts.done('Looking for artwork…');
				return api.downloadArt(album.id).then((outcome) =>
					outcome === 'notFound' ? toasts.show(artMessages[outcome]) : toasts.done(artMessages[outcome])
				);
			}
		});
	}
	items.push({
		label: 'Edit tags…',
		icon: Tags,
		run: () => tagEditor.show(api.album(album.id).then((d) => d.tracks.map((t) => t.uri)), album.title)
	});
	items.push({ label: album.art ? 'Replace artwork…' : 'Upload artwork…', icon: ImageUp, run: () => uploadArt(album) });
	return items;
}

export const albumHeader = (album: Album): MenuHeader => ({ title: album.title, subtitle: album.artist, art: album.art });

/** For a track on an album: "Play" starts the album from that track. */
export function trackItems(track: Track, albumId: string): MenuItem[] {
	return [
		{ label: 'Play', icon: Play, run: () => queue.playAlbum(albumId, track.uri) },
		{ label: 'Play next', icon: ListPlus, run: () => queue.trackNext(track.uri) },
		{ label: 'Add to end of queue', icon: ListEnd, run: () => queue.trackLast(track.uri) },
		{ label: 'Edit tags…', icon: Tags, run: () => tagEditor.show([track.uri], track.title) }
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
	const items: MenuItem[] = [{ label: 'Play', icon: Play, run: () => api.playItem(item.id) }];
	if (current >= 0 && index !== current && index !== current + 1) {
		items.push({
			label: 'Play next',
			icon: ArrowUpToLine,
			run: () => api.move(item.id, index > current ? current + 1 : current)
		});
	}
	items.push({ label: 'Edit tags…', icon: Tags, run: () => tagEditor.show([item.uri], item.title) });
	items.push({ label: 'Remove from queue', icon: ListX, run: () => api.remove(item.id), danger: true });
	return items;
}

export const queueHeader = (item: QueueItem): MenuHeader => ({ title: item.title, subtitle: item.artist, art: item.art });
