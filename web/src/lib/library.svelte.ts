import { api } from './api.ts';
import type { Album } from './types/Album.ts';

/**
 * The whole album list, fetched once per library revision (the server answers
 * 304 when unchanged) and filtered/sorted on the client.
 */
class Library {
	albums = $state.raw<Album[]>([]);
	loaded = $state(false);
	#byId = $derived(new Map(this.albums.map((a) => [a.id, a])));
	#revision = -1;

	get(id: string): Album | undefined {
		return this.#byId.get(id);
	}

	async sync(revision: number) {
		if (revision === this.#revision) return;
		this.#revision = revision;
		try {
			this.albums = await api.albums();
			this.loaded = true;
		} catch {
			this.#revision = -1;
		}
	}
}

export const library = new Library();
