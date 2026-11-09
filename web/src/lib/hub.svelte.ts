import { api } from './api.ts';
import { toasts } from './toast.svelte.ts';
import { token } from './token.ts';
import type { Downloads } from './types/Downloads.ts';
import type { HomekitStatus } from './types/HomekitStatus.ts';
import type { LibraryStatus } from './types/LibraryStatus.ts';
import type { Outputs } from './types/Outputs.ts';
import type { Party } from './types/Party.ts';
import type { PlayerState } from './types/PlayerState.ts';
import type { Queue } from './types/Queue.ts';
import type { QueueItem } from './types/QueueItem.ts';
import type { ServerEvent } from './types/ServerEvent.ts';
import type { SignalPath } from './types/SignalPath.ts';
import type { SourceState } from './types/SourceState.ts';

const idlePlayer: PlayerState = {
	online: false,
	state: 'stop',
	songId: null,
	songPos: null,
	elapsed: 0,
	duration: 0,
	bitrate: null,
	audio: null,
	volume: null,
	repeat: false,
	random: false,
	single: false,
	consume: false,
	queueVersion: 0,
	queueLength: 0,
	error: null
};

/**
 * Live daemon state over /api/ws. Each slice is replaced wholesale on every
 * event ($state.raw), which keeps big queues cheap.
 */
class Hub {
	/** WebSocket open. */
	connected = $state(false);
	/** First `hello` received. */
	ready = $state(false);
	player = $state.raw<PlayerState>(idlePlayer);
	queue = $state.raw<Queue>({ version: 0, items: [] });
	signal = $state.raw<SignalPath>({ verdict: 'idle', source: null, output: null, device: null, dop: false, note: null });
	source = $state.raw<SourceState>({ active: 'local', available: [] });
	library = $state.raw<LibraryStatus>({
		revision: 0,
		scanning: false,
		albums: 0,
		tracks: 0,
		missingArt: 0
	});
	outputs = $state.raw<Outputs>({ outputs: [], newCards: [] });
	/** The Apple Home switch; null when HomeKit is off. */
	homekit = $state.raw<HomekitStatus | null>(null);
	/** The request line: open or not, its rules, people, who is here, votes on the song playing. */
	party = $state.raw<Party>({
		open: false,
		rules: { takeTurns: true, guestsPause: true, skip: 'vote' },
		people: [],
		here: [],
		votes: { songId: null, skip: [], cheer: [], need: 1 }
	});
	/** Downloads through Lidarr and slskdpp; null when Virgil isn't set up for them. */
	downloads = $state.raw<Downloads | null>(null);

	#receivedAt = $state(0);
	#now = $state(0);
	#ticker: ReturnType<typeof setTimeout> | undefined;
	#ws: WebSocket | null = null;
	#retry = 0;
	#reconnect: ReturnType<typeof setTimeout> | undefined;

	/** Index of the current item, or -1. */
	currentIndex = $derived(
		this.player.songId == null ? -1 : this.queue.items.findIndex((i) => i.id === this.player.songId)
	);
	current = $derived<QueueItem | null>(this.queue.items[this.currentIndex] ?? null);
	playing = $derived(this.player.state === 'play');
	/** AirPlay (or another external source) owns the DAC. */
	external = $derived(this.source.active !== 'local');
	/** Playback position, interpolated between server updates. */
	elapsed = $derived.by(() => {
		const p = this.player;
		if (p.state !== 'play') return p.elapsed;
		const t = p.elapsed + Math.max(0, this.#now - this.#receivedAt) / 1000;
		return p.duration > 0 ? Math.min(p.duration, t) : t;
	});

	connect() {
		if (this.#ws || typeof window === 'undefined') return;
		const scheme = location.protocol === 'https:' ? 'wss' : 'ws';
		// The device's token, so its person counts as here while the page is open.
		const ws = new WebSocket(`${scheme}://${location.host}/api/ws?token=${token}`);
		this.#ws = ws;
		ws.onopen = () => {
			this.connected = true;
			this.#retry = 0;
		};
		ws.onmessage = (e) => this.#apply(JSON.parse(e.data) as ServerEvent);
		ws.onclose = () => {
			this.#ws = null;
			this.connected = false;
			clearTimeout(this.#reconnect);
			this.#reconnect = setTimeout(() => this.connect(), Math.min(5000, 250 * 2 ** this.#retry++));
		};
		ws.onerror = () => ws.close();
	}

	/** Phones suspend background tabs; reconnect as soon as we're visible again. */
	wake() {
		this.#tick();
		if (!this.#ws) {
			clearTimeout(this.#reconnect);
			this.#retry = 0;
			this.connect();
		}
	}

	#apply(event: ServerEvent) {
		switch (event.type) {
			case 'hello':
				this.#setPlayer(event.player);
				this.queue = event.queue;
				this.signal = event.signal;
				this.source = event.source;
				this.library = event.library;
				this.outputs = event.outputs;
				this.homekit = event.homekit;
				this.party = event.party;
				this.downloads = event.downloads;
				this.ready = true;
				break;
			case 'outputs': {
				const { type: _, ...outputs } = event;
				this.outputs = outputs;
				break;
			}
			case 'player': {
				const { type: _, ...player } = event;
				this.#setPlayer(player);
				break;
			}
			case 'queue': {
				const { type: _, ...queue } = event;
				this.queue = queue;
				break;
			}
			case 'signal': {
				const { type: _, ...signal } = event;
				this.signal = signal;
				break;
			}
			case 'source': {
				const { type: _, ...source } = event;
				this.source = source;
				break;
			}
			case 'library': {
				const { type: _, ...library } = event;
				this.library = library;
				break;
			}
			case 'homekit': {
				const { type: _, ...homekit } = event;
				this.homekit = homekit;
				break;
			}
			case 'party': {
				const { type: _, ...party } = event;
				this.party = party;
				break;
			}
			case 'downloads': {
				const { type: _, ...downloads } = event;
				this.downloads = downloads;
				break;
			}
			case 'shout':
				toasts.show(event.text, 'done', 3000);
				break;
		}
	}

	#setPlayer(player: PlayerState) {
		this.player = player;
		this.#receivedAt = performance.now();
		this.#tick();
	}

	/**
	 * Moves `elapsed` on, then again when it next reaches a whole second: what the clocks show
	 * (the progress bars glide in between). Not while paused, nor while the page is hidden.
	 */
	#tick = () => {
		clearTimeout(this.#ticker);
		this.#now = performance.now();
		if (this.player.state !== 'play' || document.hidden) return;
		const elapsed = this.player.elapsed + (this.#now - this.#receivedAt) / 1000;
		this.#ticker = setTimeout(this.#tick, (Math.floor(elapsed) + 1 - elapsed) * 1000 + 10);
	};

	// Transport, applied optimistically; the next server event confirms or corrects.

	toggle() {
		if (this.external || this.player.state === 'stop') return api.play();
		this.#setPlayer({ ...this.player, elapsed: this.elapsed, state: this.playing ? 'pause' : 'play' });
		return api.toggle();
	}

	seek(seconds: number) {
		this.#setPlayer({ ...this.player, elapsed: seconds });
		return api.seek(seconds);
	}

	/** off → repeat all → repeat one → off */
	cycleRepeat() {
		const { repeat, single } = this.player;
		const next = !repeat ? { repeat: true, single: false } : !single ? { repeat: true, single: true } : { repeat: false, single: false };
		this.player = { ...this.player, ...next };
		return api.modes(next);
	}

	toggleShuffle() {
		const random = !this.player.random;
		this.player = { ...this.player, random };
		return api.modes({ random });
	}
}

export const hub = new Hub();
