<script lang="ts">
	import { page } from '$app/state';

	// Phones: which library, in the controls row. Desktop has them in the sidebar.
	const views = [
		{ href: '/', label: 'Albums' },
		{ href: '/artists', label: 'Artists' },
		{ href: '/genres', label: 'Genres' }
	];
</script>

<nav class="flex min-w-0 flex-1 lg:hidden" aria-label="Library">
	{#each views as view (view.href)}
		{@const active = page.url.pathname === view.href}
		<a
			href={view.href}
			class="label grid min-w-0 flex-1 place-items-center border-l border-line text-[10px] first:border-l-0 {active
				? 'bg-fg text-bg'
				: 'press text-muted'}"
			aria-current={active ? 'page' : undefined}
		>
			{view.label}
		</a>
	{/each}
</nav>
