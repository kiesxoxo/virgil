<script lang="ts">
	import { flushSync } from 'svelte';
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
			} else if (e.key === 'Escape' && search.open && (e.target as HTMLElement).closest('form[role="search"]')) {
				// The bottom bar, with a hardware keyboard: Esc cancels it.
				search.clear();
				search.open = false;
			}
			return;
		}
		if (e.ctrlKey || e.metaKey || e.altKey) return;

		switch (e.key) {
			case '/':
				e.preventDefault();
				// Below desktop width the header field is hidden: open the bottom bar.
				// Either way the focus happens within the key press, or iOS ignores it.
				if (window.matchMedia('(min-width: 1024px)').matches) search.focusField();
				else {
					search.open = true;
					flushSync();
				}
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
	<div class="pointer-events-none fixed inset-0 z-[60] grid place-items-center bg-black/60">
		<div class="w-80 border border-line bg-bg" role="dialog" aria-label="Keyboard shortcuts">
			<h2 class="label flex h-9 items-center border-b border-line px-3 text-[11px] text-muted">Keyboard shortcuts</h2>
			<dl class="divide-y divide-line">
				{#each shortcuts as shortcut (shortcut.label)}
					<div class="flex h-10 items-center justify-between gap-4 px-3 text-sm">
						<dt>{shortcut.label}</dt>
						<dd class="flex gap-1">
							{#each shortcut.keys as key (key)}
								<kbd class="label min-w-7 border border-line px-1.5 py-0.5 text-center text-[10.5px]">{key}</kbd>
							{/each}
						</dd>
					</div>
				{/each}
			</dl>
			<p class="label border-t border-line px-3 py-2 text-[10px] text-muted">Release Ctrl to close</p>
		</div>
	</div>
{/if}
