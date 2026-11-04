<script lang="ts">
	import { menu } from '../menu.svelte.ts';
	import Cover from './Cover.svelte';

	const WIDTH = 232;
	const ITEM = 40;

	let width = $state(0);
	let height = $state(0);
	// Phones get a bottom sheet with big targets; larger screens a dropdown.
	let sheet = $derived(width < 640);

	let left = $derived(Math.max(8, Math.min(menu.alignRight ? menu.x - WIDTH : menu.x, width - WIDTH - 8)));
	let estimated = $derived(menu.items.length * ITEM + (menu.header ? 48 : 0) + 12);
	let top = $derived(menu.y + estimated > height - 8 ? Math.max(8, menu.yAbove - estimated) : menu.y);

	/** The finger that long-pressed lifts onto the backdrop; don't treat that as "dismiss". */
	function dismiss() {
		if (performance.now() - menu.openedAt > 350) menu.close();
	}
</script>

<svelte:window
	bind:innerWidth={width}
	bind:innerHeight={height}
	onkeydown={(e) => menu.open && e.key === 'Escape' && menu.close()}
	onresize={() => menu.close()}
/>

{#if menu.open}
	{#if sheet}
		<div class="fixed inset-0 z-50 bg-black/60" role="presentation" onclick={dismiss}></div>
		<div class="pb-safe fixed inset-x-0 bottom-0 z-50 border-t border-line bg-bg" role="menu" tabindex="-1">
			{#if menu.header}
				<div class="flex items-center border-b border-line">
					{#if menu.header.art !== undefined || menu.header.src}
						<Cover art={menu.header.art} src={menu.header.src} class="size-14 shrink-0" />
					{/if}
					<div class="min-w-0 px-3 py-2">
						<div class="wide truncate text-[13px] leading-[18px]">{menu.header.title}</div>
						{#if menu.header.subtitle}<div class="truncate text-xs leading-4 text-muted">{menu.header.subtitle}</div>{/if}
					</div>
				</div>
			{/if}
			<div class="divide-y divide-line">
				{#each menu.items as item (item.label)}
					<button
						type="button"
						role="menuitem"
						class="press flex h-12 w-full items-center gap-3.5 px-3 text-left text-[15px] font-medium {item.danger ? 'text-accent' : ''}"
						onclick={() => menu.run(item)}
					>
						<item.icon class="size-5 {item.danger ? '' : 'text-muted'}" />
						{item.label}
					</button>
				{/each}
				<button type="button" class="press wide h-14 w-full text-[13px] uppercase" onclick={() => menu.close()}>Cancel</button>
			</div>
		</div>
	{:else}
		<div
			class="fixed inset-0 z-50"
			role="presentation"
			onclick={dismiss}
			oncontextmenu={(e) => {
				e.preventDefault();
				menu.close();
			}}
		></div>
		<div
			role="menu"
			tabindex="-1"
			class="fixed z-50 divide-y divide-line border border-line bg-bg"
			style:width="{WIDTH}px"
			style:left="{left}px"
			style:top="{top}px"
		>
			{#if menu.header}
				<div class="label truncate px-3 py-2 text-[10px] text-muted">
					{menu.header.title}{menu.header.subtitle ? ` / ${menu.header.subtitle}` : ''}
				</div>
			{/if}
			{#each menu.items as item (item.label)}
				<button
					type="button"
					role="menuitem"
					class="press flex h-10 w-full items-center gap-3 px-3 text-left text-sm font-medium hover:bg-surface {item.danger ? 'text-accent' : ''}"
					onclick={() => menu.run(item)}
				>
					<item.icon class="size-4 {item.danger ? '' : 'text-muted'}" />
					{item.label}
				</button>
			{/each}
		</div>
	{/if}
{/if}
