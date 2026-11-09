// What a download is doing, in words. Pure, so node can test it (fetching.test.ts).

import type { Download } from './types/Download.ts';

type Status = Pick<Download, 'kind' | 'state' | 'progress' | 'note'>;

const percent = (progress: number) => `${Math.floor(progress * 100)}%`;

/** A word or two for a row: "Searching", "43%", "In library". */
export function badge(d: Status): string {
	switch (d.state) {
		case 'adding':
			return 'Adding';
		case 'searching':
			return 'Searching';
		case 'waiting':
			return 'Queued';
		case 'downloading':
			return d.progress == null ? 'Downloading' : percent(d.progress);
		case 'importing':
			return 'Importing';
		case 'done':
			return 'In library';
		case 'notFound':
			return 'Not found';
		case 'failed':
			return 'Failed';
	}
}

/** A line for the list in Settings: what is happening, or what went wrong. */
export function describe(d: Status): string {
	if (d.note) return d.note;
	switch (d.state) {
		case 'adding':
			return 'Lidarr is adding it';
		case 'searching':
			return d.kind === 'album' ? 'Lidarr is looking for a release' : 'Asking around on Soulseek';
		case 'waiting':
			return 'Waiting its turn';
		case 'downloading':
			return d.progress == null ? 'Downloading' : `Downloading · ${percent(d.progress)}`;
		case 'importing':
			return 'Lidarr is filing it into the library';
		case 'done':
			return 'In the library';
		case 'notFound':
			return d.kind === 'album' ? 'No release found yet' : 'Nobody has it right now';
		case 'failed':
			return 'It didn’t work';
	}
}

/** Nothing more will happen unless it is tried again. */
export const finished = (d: Pick<Download, 'state'>) => d.state === 'done' || d.state === 'notFound' || d.state === 'failed';

/** Went wrong or found nothing: worth another try. */
export const retryable = (d: Pick<Download, 'state'>) => d.state === 'notFound' || d.state === 'failed';

/** The latest download of a whole album (`track` null) or of one of its songs; `jobs` is newest first. */
export function latest<T extends Pick<Download, 'album' | 'track'>>(jobs: T[], album: string, track: number | null = null): T | undefined {
	return jobs.find((d) => d.album === album && d.track === track);
}
