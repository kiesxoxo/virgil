// The night on the request line, told back: its set list and its awards.

import { span } from './format.ts';
import type { Aircheck } from './types/Aircheck.ts';
import type { Person } from './types/Person.ts';
import type { Played } from './types/Played.ts';

export interface Award {
	title: string;
	person: Person | null;
	detail: string;
}

/** What played, one entry per request (an album stays one line). */
export interface SetEntry {
	songs: Played[];
	person: Person | null;
	/** Two or more songs from one album. */
	album: boolean;
}

export function setList(night: Aircheck): SetEntry[] {
	const people = new Map(night.people.map((p) => [p.id, p]));
	const out: SetEntry[] = [];
	for (const song of night.played) {
		const last = out.at(-1);
		const request = song.pick?.request;
		if (last && request != null && last.songs[0].pick?.request === request) last.songs.push(song);
		else out.push({ songs: [song], person: (song.pick && people.get(song.pick.by)) ?? null, album: false });
	}
	for (const entry of out) {
		const album = entry.songs[0].albumId;
		entry.album = entry.songs.length > 1 && album != null && entry.songs.every((s) => s.albumId === album);
	}
	return out;
}

/** Who stood out. Only awards someone won are given. */
export function awards(night: Aircheck): Award[] {
	const people = new Map(night.people.map((p) => [p.id, p]));
	const picked = night.played.filter((s) => s.pick && people.has(s.pick.by));
	const who = (s: Played) => people.get(s.pick!.by) ?? null;
	const out: Award[] = [];

	const first = picked.reduce<Played | null>((a, s) => (!a || s.pick!.at < a.pick!.at ? s : a), null);
	if (first) out.push({ title: 'First caller', person: who(first), detail: first.title });

	const requests = new Map<string, Set<number>>();
	const minutes = new Map<string, number>();
	for (const s of picked) {
		const by = s.pick!.by;
		requests.set(by, (requests.get(by) ?? new Set()).add(s.pick!.request));
		minutes.set(by, (minutes.get(by) ?? 0) + s.duration);
	}
	const top = (m: Map<string, number>) => [...m].reduce<[string, number] | null>((a, e) => (!a || e[1] > a[1] ? e : a), null);
	const most = top(new Map([...requests].map(([by, r]) => [by, r.size])));
	if (most && most[1] > 1) out.push({ title: 'Most requests', person: people.get(most[0]) ?? null, detail: `${most[1]} requests` });

	const favourite = picked.reduce<Played | null>((a, s) => (s.cheers > (a?.cheers ?? 0) ? s : a), null);
	if (favourite) out.push({ title: 'Crowd favourite', person: who(favourite), detail: `${favourite.title} · ♥ ${favourite.cheers}` });

	const longest = top(minutes);
	if (longest && people.size > 1) out.push({ title: 'Longest set', person: people.get(longest[0]) ?? null, detail: span(longest[1]) });

	const brave = picked.reduce<Played | null>((a, s) => (s.skipVotes > (a?.skipVotes ?? 0) ? s : a), null);
	if (brave) {
		const votes = `${brave.skipVotes} skip vote${brave.skipVotes === 1 ? '' : 's'}`;
		out.push({ title: 'Bravest pick', person: who(brave), detail: `${brave.title} · ${votes}` });
	}
	return out;
}

/** "5 callers · 63 songs · 4 hr 12 min". */
export function summary(night: Aircheck): string {
	const callers = new Set(night.played.flatMap((s) => (s.pick ? [s.pick.by] : []))).size;
	const end = night.closed ?? night.played.at(-1)?.at ?? night.opened;
	const parts = [`${callers} caller${callers === 1 ? '' : 's'}`, `${night.played.length} song${night.played.length === 1 ? '' : 's'}`];
	// Under a minute isn't worth saying.
	if (end - night.opened >= 60) parts.push(span(end - night.opened));
	return parts.join(' · ');
}

/** The set list as text, for the group chat. */
export function setListText(night: Aircheck, time: (unix: number) => string): string {
	const lines = [`Aircheck · ${summary(night)}`, ''];
	for (const entry of setList(night)) {
		const first = entry.songs[0];
		const what = entry.album ? `${first.album} (album, ${entry.songs.length} songs)` : entry.songs.map((s) => s.title).join(' / ');
		const by = entry.person ? ` · ${entry.person.name}` : '';
		const note = first.pick?.note ? ` · “${first.pick.note}”` : '';
		lines.push(`${time(first.at)}  ${first.artist} — ${what}${by}${note}`);
	}
	const won = awards(night);
	if (won.length) {
		lines.push('');
		for (const a of won) lines.push(`${a.title}: ${a.person?.name ?? '?'} (${a.detail})`);
	}
	return lines.join('\n');
}
