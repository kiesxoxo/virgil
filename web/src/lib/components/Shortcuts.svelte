<script lang="ts">
	import { fade, scale } from 'svelte/transition';
	import { goto } from '$app/navigation';
	import { api } from '../api.ts';
	import { hub } from '../hub.svelte.ts';
	import { menu } from '../menu.svelte.ts';
	import { search } from '../search.svelte.ts';
	import { tagEditor } from '../tageditor.svelte.ts';

	// A few keyboard shortcuts, and a cheat sheet while Ctrl is held on its own.
	const shortcuts: { keys: string[]; label: string }[] = [
		{ keys: ['/'], label: 'Search' },
		{ keys: ['Space'], label: 'Play / pause' },
		{ keys: ['N'], label: 'Next track' },
		{ keys: ['P'], label: 'Previous track' },
		{ keys: ['Q'], label: 'Queue' },
		{ keys: ['Esc'], label: 'Clear search' }
	];

	/** How long Ctrl has to be held alone before the cheat sheet shows. */
	const HOLD_MS = 450;

	let sheet = $state(false);
	let holding: ReturnType<typeof setTimeout> | undefined;

	const editable = (el: EventTarget | null) =>
		el instanceof HTMLElement && (el.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(el.tagName));
	// Space on a focused button, link or slider belongs to it.
	const interactive = (el: EventTarget | null) =>
		el instanceof HTMLElement && !!el.closest('button, a, [role="slider"], [role="button"], [role="radio"]');

	function hide() {
		clearTimeout(holding);
		holding = undefined;
		sheet = false;
	}

	function keydown(e: KeyboardEvent) {
		if (e.key === 'Control') {
			if (!e.repeat) holding = setTimeout(() => (sheet = true), HOLD_MS);
			return;
		}
		// Ctrl+C and friends aren't a request for the cheat sheet.
		hide();
		if (e.defaultPrevented || tagEditor.open || menu.open) return;

		if (editable(e.target)) {
			if (e.key === 'Escape' && search.field && e.target === search.field) {
				search.clear();
				search.field.blur();
			}
			return;
		}
		if (e.ctrlKey || e.metaKey || e.altKey) return;

		switch (e.key) {
			case '/':
				e.preventDefault();
				if (search.field || window.matchMedia('(min-width: 1024px)').matches) search.focusField();
				else search.open = true;
				break;
			case ' ':
				if (interactive(e.target) || !hub.player.online) return;
				e.preventDefault();
				hub.toggle();
				break;
			case 'n':
			case 'N':
				api.next();
				break;
			case 'p':
			case 'P':
				api.previous();
				break;
			case 'q':
			case 'Q':
				goto('/queue');
				break;
			case 'Escape':
				if (search.query) search.clear();
				break;
		}
	}
</script>

<svelte:window
	onkeydown={keydown}
	onkeyup={(e) => e.key === 'Control' && hide()}
	onblur={hide}
/>

{#if sheet}
	<div class="pointer-events-none fixed inset-0 z-[60] grid place-items-center bg-black/30" transition:fade={{ duration: 120 }}>
		<div
			class="glass w-80 rounded-3xl p-5 shadow-float ring-1 ring-line"
			role="dialog"
			aria-label="Keyboard shortcuts"
			transition:scale={{ start: 0.96, duration: 140 }}
		>
			<h2 class="text-sm font-semibold">Keyboard shortcuts</h2>
			<dl class="mt-3 space-y-2">
				{#each shortcuts as shortcut (shortcut.label)}
					<div class="flex items-center justify-between gap-4 text-sm">
						<dt class="text-muted">{shortcut.label}</dt>
						<dd class="flex gap-1">
							{#each shortcut.keys as key (key)}
								<kbd class="min-w-7 rounded-md bg-surface px-2 py-0.5 text-center font-sans text-xs font-semibold ring-1 ring-line">{key}</kbd>
							{/each}
						</dd>
					</div>
				{/each}
			</dl>
			<p class="mt-4 text-xs text-faint">Release Ctrl to close</p>
		</div>
	</div>
{/if}
