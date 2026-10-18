<script lang="ts">
	import { page } from '$app/state';

	// Phones: switch between the library's views. Desktop has them in the sidebar.
	const tabs = [
		{ href: '/', label: 'Albums' },
		{ href: '/artists', label: 'Artists' },
		{ href: '/genres', label: 'Genres' }
	];
</script>

<nav class="mb-3 grid grid-cols-3 gap-1 rounded-xl bg-surface p-1 lg:hidden" aria-label="Library">
	{#each tabs as tab (tab.href)}
		{@const active = page.url.pathname === tab.href}
		<a
			href={tab.href}
			class="grid h-8 place-items-center rounded-lg text-[13px] font-semibold transition-colors {active
				? 'bg-bg text-fg shadow-sm ring-1 ring-line'
				: 'text-muted'}"
			aria-current={active ? 'page' : undefined}
		>
			{tab.label}
		</a>
	{/each}
</nav>
