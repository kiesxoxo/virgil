// The request line on this device: who it is, what it may do, and the moments worth a toast.

import { page } from '$app/state';
import { api, ApiError } from './api.ts';
import { hub } from './hub.svelte.ts';
import { freeColor, handle, signFor } from './names.ts';
import { isBoolean, store, stored } from './stored.ts';
import { toasts } from './toast.svelte.ts';
import type { Person } from './types/Person.ts';
import type { Pick } from './types/Pick.ts';
import type { QueueItem } from './types/QueueItem.ts';

/** What a dedication sheet is for: its title, and what requesting with the note does. */
export interface Dedication {
	title: string;
	subtitle?: string;
	send: (note: string) => unknown;
}

class Line {
	/** This device's person, as virgild last said. */
	me = $state.raw<Person | null>(null);
	/** The person on this device picked their name (the join sheet, or Settings). */
	confirmed = $state(stored('whoConfirmed', isBoolean, false));
	/** The join sheet is open. */
	joining = $state(false);
	/** The dedication sheet, while it is open. */
	dedication = $state.raw<Dedication | null>(null);
	/** The request line sheet (invite, people, rules) is open. */
	sheet = $state(false);

	people = $derived(new Map(hub.party.people.map((p) => [p.id, p])));
	/** `me` as everyone sees it now (a host may have promoted this device). */
	self = $derived(this.me ? (this.people.get(this.me.id) ?? this.me) : null);
	open = $derived(hub.party.open);
	/** May do everything: with the line closed, everyone may. */
	host = $derived(!hub.party.open || (this.self?.host ?? false));
	canPause = $derived(this.host || hub.party.rules.guestsPause);
	/** What the Next key does here: skip, vote to skip, or nothing. */
	skipKey = $derived<'next' | 'vote' | 'none'>(
		this.host || hub.party.rules.skip === 'free' || this.mine(hub.current)
			? 'next'
			: hub.party.rules.skip === 'vote'
				? 'vote'
				: 'none'
	);
	here = $derived(hub.party.here.flatMap((id) => this.people.get(id) ?? []));
	/** Call signs show once two people share the queue, or the line is open. */
	signs = $derived(hub.party.open || new Set(hub.queue.items.flatMap((i) => (i.pick ? [i.pick.by] : []))).size > 1);
	votedSkip = $derived(!!this.me && hub.party.votes.skip.includes(this.me.id));
	cheered = $derived(!!this.me && hub.party.votes.cheer.includes(this.me.id));

	#starting = false;

	/** Whoever asked for `pick`. */
	person(pick: Pick | null | undefined): Person | undefined {
		return pick ? this.people.get(pick.by) : undefined;
	}

	mine(item: QueueItem | null | undefined): boolean {
		return !!item?.pick && !!this.me && item.pick.by === this.me.id;
	}

	/**
	 * Learns who this device is, or says so the first time: a random handle, a free colour. Devices
	 * that turn up while the line is closed become hosts, so the household's own phones are.
	 */
	async start() {
		if (this.me || this.#starting) return;
		this.#starting = true;
		try {
			this.me = await api.me();
		} catch (e) {
			if (!(e instanceof ApiError && e.status === 404)) {
				this.#starting = false;
				return;
			}
			const name = handle();
			const people = hub.party.people;
			this.me = await api
				.setMe({ name, sign: signFor(name, people.map((p) => p.sign)), color: freeColor(people.map((p) => p.color)) })
				.catch(() => null);
		}
		this.#starting = false;
		if (this.open && !this.confirmed) this.joining = true;
	}

	/** Saves who this device is. From the join sheet, the room hears that someone called in. */
	async save(name: string, sign: string, color: number, join = false) {
		this.me = await api.setMe({ name, sign, color, join });
		this.confirmed = true;
		store({ whoConfirmed: true });
	}

	async link(id: string) {
		this.me = await api.linkMe(id);
		this.confirmed = true;
		store({ whoConfirmed: true });
	}

	/** Opens or closes the request line. */
	async setOpen(open: boolean) {
		await api.party({ open });
		if (open && !this.confirmed) this.joining = true;
	}

	/** The Next key: hosts skip; guests skip their own request, or vote, as the rules say. */
	skip() {
		return this.host ? api.next() : api.skip();
	}

	dedicate(dedication: Dedication) {
		this.dedication = dedication;
	}

	/**
	 * Watches for this device's moments: its request coming on, or being next. Call once, from the
	 * layout.
	 */
	watch() {
		let lastOn: number | null = null;
		let lastNext: number | null = null;
		$effect(() => {
			const current = hub.current;
			const next = hub.queue.items[hub.currentIndex + 1];
			if (!this.open || !hub.playing) return;
			if (current && current.id !== lastOn && this.mine(current)) toasts.done('Your request is on');
			lastOn = current?.id ?? null;
			// Only the first song of a request: the rest of an album isn't news.
			const starts = next && this.mine(next) && next.pick?.request !== current?.pick?.request;
			if (next && next.id !== lastNext && starts) toasts.done('You’re up next');
			lastNext = next?.id ?? null;
		});
		$effect(() => {
			// The big screen is for the room, not a person: it doesn't sign itself in, so it doesn't count.
			if (hub.ready && page.route.id !== '/screen') void this.start();
		});
		$effect(() => {
			if (this.open && this.me && !this.confirmed) this.joining = true;
		});
	}
}

export const line = new Line();

/** "1st", "2nd", "3rd", "11th". */
export function ordinal(n: number): string {
	const teen = n % 100 >= 11 && n % 100 <= 13;
	const suffix = teen ? 'th' : (['th', 'st', 'nd', 'rd'][n % 10] ?? 'th');
	return `${n}${suffix}`;
}
