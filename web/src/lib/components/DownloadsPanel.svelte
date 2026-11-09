<script lang="ts">
	// Settings › Downloads: everything being fetched, and what came of it lately.
	import { RotateCw, X } from '@lucide/svelte';
	import { downloads } from '../downloads.svelte.ts';
	import { describe, finished, retryable } from '../fetching.ts';
	import { line } from '../party.svelte.ts';
	import Cover from './Cover.svelte';
	import Sign from './Sign.svelte';
</script>

{#if !downloads.jobs.length}
	<p class="border-b border-line px-3 py-2.5 text-[13px] text-muted">
		Search the library for something it doesn’t have, or open an artist or an album with songs missing, to download from
		Lidarr{downloads.songs ? ' and Soulseek' : ''}.
	</p>
{/if}
{#each downloads.jobs as job (job.id)}
	{@const done = finished(job)}
	<div class="flex items-stretch border-b border-line">
		<Cover art={null} src={job.cover} class="w-14 shrink-0 self-stretch" />
		<div class="min-w-0 flex-1 px-3 py-2">
			<div class="flex items-center gap-1.5">
				<Sign person={job.by ? line.people.get(job.by) : null} />
				<span class="truncate text-sm font-medium">{job.title}</span>
			</div>
			<div class="truncate text-xs text-muted">{job.kind === 'song' ? `${job.artist} · ${job.albumTitle}` : job.artist}</div>
			<div class="label mt-1 truncate text-[10.5px] {job.state === 'failed' ? 'text-warn' : done ? 'text-muted' : 'text-accent'}">{describe(job)}</div>
			{#if job.progress != null && !done}
				<div class="mt-1.5 h-[3px] bg-surface-2"><div class="h-full bg-accent" style:width="{job.progress * 100}%"></div></div>
			{/if}
		</div>
		{#if job.state === 'done' && job.albumId}
			<a href="/album/{job.albumId}" class="press label flex shrink-0 items-center border-l border-line px-3 text-[10.5px]">Open</a>
		{/if}
		{#if retryable(job)}
			<button type="button" class="press grid w-11 shrink-0 place-items-center border-l border-line" title="Try again" aria-label="Try again" onclick={() => downloads.retry(job.id)}>
				<RotateCw class="size-4" />
			</button>
		{/if}
		<button
			type="button"
			class="press grid w-11 shrink-0 place-items-center border-l border-line text-muted"
			title={done ? 'Remove from the list' : job.kind === 'song' ? 'Stop' : 'Remove from the list (Lidarr carries on)'}
			aria-label={done ? 'Remove from the list' : 'Stop'}
			onclick={() => downloads.dismiss(job.id)}
		>
			<X class="size-4" />
		</button>
	</div>
{/each}
