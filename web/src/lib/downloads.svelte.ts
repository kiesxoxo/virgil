// Getting music the library doesn't have: albums through Lidarr, single songs through slskdpp.
// virgild does the work and reports on the WebSocket (hub.downloads); this starts things and keeps
// the catalogue sheet.

import { api } from './api.ts';
import { latest } from './fetching.ts';
import { hub } from './hub.svelte.ts';
import { toasts } from './toast.svelte.ts';
import type { CatalogAlbum } from './types/CatalogAlbum.ts';
import type { CatalogArtist } from './types/CatalogArtist.ts';
import type { Download } from './types/Download.ts';

class Downloads {
	/** The album open in the catalogue sheet. */
	sheet = $state.raw<CatalogAlbum | null>(null);
	/** The artist open in the artist sheet (one the library doesn't have). */
	artist = $state.raw<CatalogArtist | null>(null);

	/** Virgil is set up to download (Lidarr). */
	get on(): boolean {
		return hub.downloads !== null;
	}

	/** Single songs too (slskdpp). */
	get songs(): boolean {
		return hub.downloads?.songs ?? false;
	}

	/** Newest first. */
	get jobs(): Download[] {
		return hub.downloads?.jobs ?? [];
	}

	/** The latest download of an album (whole), or of one of its songs. */
	of(album: string, track: number | null = null): Download | undefined {
		return latest(this.jobs, album, track);
	}

	async album(album: string) {
		await api.download({ album });
		toasts.done('Lidarr is on it');
	}

	async song(album: string, track: number) {
		await api.download({ album, track });
		toasts.done('Looking for it on Soulseek');
	}

	retry(id: number) {
		return api.retryDownload(id);
	}

	dismiss(id: number) {
		return api.dismissDownload(id);
	}

	open(album: CatalogAlbum) {
		this.sheet = album;
	}

	close() {
		this.sheet = null;
	}

	openArtist(artist: CatalogArtist) {
		this.artist = artist;
	}

	closeArtist() {
		this.artist = null;
	}
}

export const downloads = new Downloads();
