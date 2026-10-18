import { api } from './api.ts';
import { artists, genres, type Group } from './browse.ts';
import { bare, fold, qualityRank } from './format.ts';
import type { Album } from './types/Album.ts';

/**
 * Albums that are versions of one another share this key: same artist and
 * title once case, accents, punctuation and bracketed tags such as "[WEB]",
 * "(2014)" or "(Remastered)" are ignored.
 */
export function versionKey(album: Album): string {
	// A title that is all brackets ("[Untitled]") keeps its text.
	const clean = (s: string) => bare(s) || fold(s).trim();
	return `${clean(album.artist)}\u001f${clean(album.title)}`;
}

/** Most complete first, then highest resolution, then newest. */
function better(a: Album, b: Album): number {
	return b.tracks - a.tracks || qualityRank(b.format) - qualityRank(a.format) || b.added - a.added;
}

/**
 * The whole album list, fetched once per library revision (the server answers
 * 304 when unchanged) and filtered/sorted on the client.
 */
class Library {
	albums = $state.raw<Album[]>([]);
	loaded = $state(false);
	#revision = -1;

	#byId = $derived(new Map(this.albums.map((a) => [a.id, a])));

	/** Versions per album id (best first), and one entry per set of versions in server order. */
	#grouped = $derived.by(() => {
		const groups = new Map<string, Album[]>();
		const order: string[] = [];
		for (const album of this.albums) {
			const key = versionKey(album);
			const group = groups.get(key);
			if (group) group.push(album);
			else {
				groups.set(key, [album]);
				order.push(key);
			}
		}
		const versions = new Map<string, Album[]>();
		const entries: Album[] = [];
		for (const key of order) {
			const group = groups.get(key)!.toSorted(better);
			for (const album of group) versions.set(album.id, group);
			entries.push(group[0]);
		}
		return { versions, entries };
	});

	/** The albums to show: duplicates collapsed into their best version. */
	get entries(): Album[] {
		return this.#grouped.entries;
	}

	#artists = $derived(artists(this.#grouped.entries));
	#genres = $derived(genres(this.#grouped.entries));

	/** One entry per album artist, A–Z. */
	get artists(): Group[] {
		return this.#artists;
	}

	/** One entry per genre, A–Z, with "No genre" last. */
	get genres(): Group[] {
		return this.#genres;
	}

	get(id: string): Album | undefined {
		return this.#byId.get(id);
	}

	/** Every version of the album with this id, best first (just the album when it is unique). */
	versions(id: string): Album[] {
		return this.#grouped.versions.get(id) ?? [];
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
