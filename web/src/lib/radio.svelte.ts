// NTS radio: the stations from virgild (/api/radio), kept current as shows
// change, and how a stream in the queue is shown: as the show on air or the
// mixtape, with its artwork, instead of a bare URL.

import type { QueueItem } from './types/QueueItem.ts';
import type { Radio } from './types/Radio.ts';
import type { Station } from './types/Station.ts';

/** An internet stream rather than a file from the library. */
export const isStream = (uri: string) => /^https?:\/\//.test(uri);

/** A queue item as lists, the player bar and Now Playing show it. */
export interface Shown {
	title: string;
	subtitle: string;
	/** Library artwork key (Cover's `art`)… */
	art: string | null;
	/** …or image URLs from NTS (Cover's `src`): small, and big for Now Playing. */
	src: string | null;
	large: string | null;
	/** Set for a stream from a station Virgil knows. */
	station: Station | null;
}

/** Re-read the schedule this long after a show ends, giving NTS time to move on. */
const AFTER_SHOW = 10_000;

class RadioStore {
	data = $state.raw<Radio | null>(null);
	/** Why the stations couldn't be loaded; cleared once they are. */
	error = $state<string | null>(null);
	/** Unix seconds, ticking every few seconds so show progress moves. */
	now = $state(Math.floor(Date.now() / 1000));

	#byStream = $derived(new Map(this.data ? [...this.data.live, ...this.data.mixtapes].map((s) => [s.stream, s]) : []));
	#loading: Promise<void> | null = null;
	#refresh: ReturnType<typeof setTimeout> | undefined;
	#ticker: ReturnType<typeof setInterval> | undefined;

	station(uri: string): Station | undefined {
		return this.#byStream.get(uri);
	}

	/** Fetches the stations once (later calls reuse them, or retry after a failure). */
	load(): Promise<void> {
		if (this.data) return Promise.resolve();
		return (this.#loading ??= this.#fetch());
	}

	async #fetch() {
		try {
			const res = await fetch('/api/radio');
			if (!res.ok) throw new Error((await res.json().catch(() => null))?.error ?? res.statusText);
			this.data = (await res.json()) as Radio;
			this.error = null;
		} catch (e) {
			this.error = (e as Error).message || 'Radio is unavailable';
		} finally {
			this.#loading = null;
			this.#schedule();
		}
	}

	/** Next fetch: when the first show on air ends (within 15 s – 10 min). */
	#schedule() {
		clearTimeout(this.#refresh);
		const ends = (this.data?.live ?? []).flatMap((s) => (s.now ? [s.now.end * 1000] : []));
		const wait = ends.length ? Math.min(...ends) - Date.now() + AFTER_SHOW : 60_000;
		this.#refresh = setTimeout(() => (this.#loading ??= this.#fetch()), Math.min(Math.max(wait, 15_000), 600_000));
		this.#ticker ??= setInterval(() => (this.now = Math.floor(Date.now() / 1000)), 5_000);
	}

	/** How to show a queue item: a stream as its station, anything else as itself. */
	show(item: QueueItem): Shown {
		if (!isStream(item.uri)) return { title: item.title, subtitle: item.artist, art: item.art, src: null, large: null, station: null };
		const station = this.station(item.uri) ?? null;
		if (!station) {
			const host = item.uri.replace(/^https?:\/\/([^/]+).*/, '$1');
			return { title: 'Internet radio', subtitle: host, art: null, src: null, large: null, station };
		}
		const show = station.now;
		return {
			title: show?.title ?? station.name,
			subtitle: show ? [station.name, show.location].filter(Boolean).join(' · ') : station.tagline || 'NTS Infinite Mixtape',
			art: null,
			src: station.thumb ?? station.art,
			large: station.art ?? station.thumb,
			station
		};
	}

	/** How far through its show a live channel is, 0–1; null for a mixtape. */
	progress(station: Station): number | null {
		const show = station.now;
		if (!show || show.end <= show.start) return null;
		return Math.min(1, Math.max(0, (this.now - show.start) / (show.end - show.start)));
	}
}

export const radio = new RadioStore();
