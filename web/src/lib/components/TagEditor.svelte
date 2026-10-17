<script lang="ts">
	import {
		CaseLower,
		CaseSensitive,
		CaseUpper,
		Eraser,
		FileText,
		Hash,
		LoaderCircle,
		Pencil,
		Plus,
		RotateCcw,
		Trash2,
		TriangleAlert,
		Wand2,
		X
	} from '@lucide/svelte';
	import { fade, fly } from 'svelte/transition';
	import { clock, khz } from '../format.ts';
	import { menu, type MenuItem } from '../menu.svelte.ts';
	import { LONG, STANDARD, changed, fillFromName, shared, validName, type Row } from '../tagedit.ts';
	import { tagEditor as editor } from '../tageditor.svelte.ts';
	import Menu from './Menu.svelte';

	let tab = $state<'metadata' | 'details'>('metadata');
	let renaming = $state<number | null>(null);
	let pattern = $state<string | null>(null);

	$effect(() => {
		if (!editor.open) {
			tab = 'metadata';
			renaming = null;
			pattern = null;
		}
	});

	let count = $derived(editor.files.length);
	let subtitle = $derived(
		editor.loading ? 'Reading tags…' : `${editor.title} · ${count} ${count === 1 ? 'file' : 'files'}`
	);

	function rowItems(row: Row): MenuItem[] {
		const items: MenuItem[] = [];
		if (row.removed) return [{ label: 'Restore', icon: RotateCcw, run: () => (row.removed = false) }];
		items.push(
			{ label: 'Rename', icon: Pencil, run: () => (renaming = row.id) },
			{ label: 'Title Case', icon: CaseSensitive, run: () => editor.recase(row, 'title') },
			{ label: 'UPPERCASE', icon: CaseUpper, run: () => editor.recase(row, 'upper') },
			{ label: 'lowercase', icon: CaseLower, run: () => editor.recase(row, 'lower') },
			{ label: 'Trim spaces', icon: Eraser, run: () => editor.recase(row, 'trim') }
		);
		if (changed(row) && row.original !== undefined) {
			items.push({
				label: 'Undo changes',
				icon: RotateCcw,
				run: () => {
					row.name = row.original!;
					row.text = row.before.map((v) => v.join('; '));
				}
			});
		}
		items.push({ label: 'Remove field', icon: Trash2, run: () => editor.remove(row), danger: true });
		return items;
	}

	function tools(button: HTMLElement) {
		menu.show(
			[
				{ label: 'Auto track number', icon: Hash, run: () => editor.autoNumber() },
				{ label: 'Fill from file names…', icon: FileText, run: () => (pattern ??= '%tracknumber% - %title%') }
			],
			button.getBoundingClientRect()
		);
	}

	function addField() {
		const row = editor.add();
		renaming = row.id;
	}

	let preview = $derived(
		pattern === null ? [] : editor.files.slice(0, 50).map((f) => ({ uri: f.uri, fields: fillFromName(pattern!, f.uri) }))
	);

	function applyPattern() {
		if (pattern === null) return;
		const matched = editor.fill(pattern);
		if (matched) pattern = null;
	}

	const focus = (node: HTMLElement) => {
		node.focus();
		if (node instanceof HTMLInputElement) node.select();
	};

	/** Grows a textarea with its content. */
	const grow = (node: HTMLTextAreaElement) => {
		const fit = () => {
			node.style.height = 'auto';
			node.style.height = `${node.scrollHeight + 2}px`;
		};
		fit();
		node.addEventListener('input', fit);
		return () => node.removeEventListener('input', fit);
	};

	/** foobar2000's friendly name for standard fields, else the field name. */
	const labelOf = (row: Row) => STANDARD.find(([key]) => key === row.name.toUpperCase())?.[1] ?? row.name;

	const fileName = (uri: string) => uri.split('/').pop() ?? uri;
	const bytes = (n: number) => (n >= 1 << 30 ? `${(n / 2 ** 30).toFixed(2)} GB` : `${(n / 2 ** 20).toFixed(1)} MB`);
	let totals = $derived.by(() => {
		const infos = editor.files.map((f) => f.info!).filter(Boolean);
		return {
			duration: infos.reduce((s, i) => s + i.duration, 0),
			size: infos.reduce((s, i) => s + i.size, 0),
			codecs: [...new Set(infos.map((i) => i.codec))].join(', '),
			tags: [...new Set(infos.map((i) => i.tagType))].join(', ')
		};
	});
</script>

<svelte:window
	onkeydown={(e) => {
		if (!editor.open || menu.open) return;
		if (e.key === 'Escape') editor.close();
		if (e.key === 's' && (e.metaKey || e.ctrlKey)) {
			e.preventDefault();
			if (editor.dirty && !editor.problem) editor.save();
		}
	}}
/>

{#snippet detail(label: string, value: string | number | null | undefined)}
	{#if value !== null && value !== undefined && value !== ''}
		<div class="flex justify-between gap-4 px-4 py-2.5 text-sm">
			<dt class="text-muted">{label}</dt>
			<dd class="min-w-0 text-right font-medium break-words tabular-nums">{value}</dd>
		</div>
	{/if}
{/snippet}

{#if editor.open}
	<div class="fixed inset-0 z-40 bg-black/45 max-sm:hidden" role="presentation" transition:fade={{ duration: 150 }} onclick={() => editor.close()}></div>
	<div
		class="fixed inset-0 z-40 flex flex-col bg-bg sm:inset-x-4 sm:top-[6vh] sm:bottom-auto sm:mx-auto sm:h-[min(88vh,52rem)] sm:max-w-3xl sm:rounded-3xl sm:shadow-float sm:ring-1 sm:ring-line"
		role="dialog"
		aria-modal="true"
		aria-label="Tag editor"
		transition:fly={{ y: 40, duration: 200 }}
	>
		<header class="pt-safe flex items-center gap-2 border-b border-line px-2 sm:px-3">
			<button type="button" class="press h-11 rounded-full px-3 text-[15px] font-medium text-accent" disabled={editor.saving} onclick={() => editor.close()}>
				Cancel
			</button>
			<div class="min-w-0 flex-1 py-2.5 text-center">
				<h2 class="font-semibold">Properties</h2>
				<p class="truncate text-xs text-muted">{subtitle}</p>
			</div>
			<button
				type="button"
				class="press inline-flex h-9 items-center gap-1.5 rounded-full bg-accent px-4 text-sm font-semibold text-accent-fg disabled:opacity-40"
				disabled={!editor.dirty || !!editor.problem || editor.saving}
				title={editor.problem ?? undefined}
				onclick={() => editor.save()}
			>
				{#if editor.saving}<LoaderCircle class="size-4 animate-spin" />{/if}
				Save
			</button>
		</header>

		<div class="flex items-center gap-2 px-3 pt-3 sm:px-4">
			<div class="grid flex-1 grid-cols-2 gap-1 rounded-xl bg-surface p-1 sm:max-w-64">
				{#each [['metadata', 'Metadata'], ['details', 'Details']] as [id, label] (id)}
					<button
						type="button"
						class="h-8 rounded-lg text-[13px] font-semibold {tab === id ? 'bg-bg text-fg shadow-sm ring-1 ring-line' : 'text-muted'}"
						aria-pressed={tab === id}
						onclick={() => (tab = id as typeof tab)}
					>
						{label}
					</button>
				{/each}
			</div>
			{#if tab === 'metadata'}
				<button
					type="button"
					class="press inline-flex h-10 items-center gap-1.5 rounded-full bg-surface px-3.5 text-[13px] font-semibold disabled:opacity-40"
					disabled={!count || editor.saving}
					onclick={(e) => tools(e.currentTarget)}
				>
					<Wand2 class="size-4" /> Tools
				</button>
			{/if}
		</div>

		<div class="min-h-0 flex-1 overflow-y-auto overscroll-contain px-3 pt-3 pb-[calc(1.5rem+env(safe-area-inset-bottom))] sm:px-4">
			{#if editor.loading}
				<div class="grid h-40 place-items-center text-muted"><LoaderCircle class="size-6 animate-spin" /></div>
			{:else}
				{#if editor.unreadable.length}
					<div class="mb-3 flex gap-2.5 rounded-2xl bg-warn-bg px-4 py-3 text-[13px] text-warn">
						<TriangleAlert class="mt-0.5 size-4 shrink-0" />
						<div class="min-w-0">
							<p class="font-semibold">
								{editor.unreadable.length}
								{editor.unreadable.length === 1 ? 'file is' : 'files are'} left out
							</p>
							{#each editor.unreadable.slice(0, 3) as file (file.uri)}
								<p class="truncate">{fileName(file.uri)}: {file.error}</p>
							{/each}
						</div>
					</div>
				{/if}

				{#if tab === 'metadata' && count}
					{#if pattern !== null}
						<section class="mb-3 rounded-2xl bg-surface p-3" transition:fly={{ y: -8, duration: 150 }}>
							<div class="flex items-center justify-between">
								<h3 class="text-sm font-semibold">Fill from file names</h3>
								<button type="button" class="grid size-8 place-items-center rounded-full text-muted hover:bg-fg/6" aria-label="Close" onclick={() => (pattern = null)}>
									<X class="size-4" />
								</button>
							</div>
							<input
								class="mt-2 h-10 w-full rounded-xl bg-bg px-3 font-mono text-base ring-1 ring-line outline-none focus:ring-2 focus:ring-accent sm:text-sm"
								bind:value={pattern}
								spellcheck="false"
								autocapitalize="off"
								autocomplete="off"
								{@attach focus}
								onkeydown={(e) => e.key === 'Enter' && applyPattern()}
							/>
							<p class="mt-1.5 text-xs text-muted">
								Fields like <code>%title%</code>, <code>%tracknumber%</code>, <code>%artist%</code>; <code>%_%</code> skips a part,
								<code>/</code> reaches into folders.
							</p>
							<ul class="mt-2.5 max-h-48 space-y-1 overflow-y-auto text-xs">
								{#each preview as item (item.uri)}
									<li class="rounded-lg bg-bg px-2.5 py-1.5">
										<div class="truncate text-muted">{fileName(item.uri)}</div>
										{#if item.fields}
											<div class="truncate font-medium">
												{Object.entries(item.fields)
													.map(([k, v]) => `${k}: ${v}`)
													.join(' · ') || '—'}
											</div>
										{:else}
											<div class="text-warn">Doesn't match</div>
										{/if}
									</li>
								{/each}
							</ul>
							<div class="mt-2.5 flex justify-end">
								<button
									type="button"
									class="press h-9 rounded-full bg-fg px-4 text-[13px] font-semibold text-bg disabled:opacity-40"
									disabled={!preview.some((p) => p.fields)}
									onclick={applyPattern}
								>
									Fill fields
								</button>
							</div>
						</section>
					{/if}

					<ul class="divide-y divide-line rounded-2xl bg-surface">
						{#each editor.rows as row (row.id)}
							{@const value = shared(row)}
							{@const long = LONG.has(row.name.toUpperCase())}
							<li class="flex gap-2 py-1.5 pr-1 pl-3 sm:items-start max-sm:flex-col max-sm:gap-1">
								<div class="flex min-h-9 items-center gap-1.5 sm:w-44 sm:shrink-0">
									{#if changed(row)}<span class="size-1.5 shrink-0 rounded-full bg-accent" title="Changed"></span>{/if}
									{#if renaming === row.id}
										<input
											class="h-8 w-full min-w-0 rounded-lg bg-bg px-2 text-base font-semibold uppercase ring-1 outline-none focus:ring-2 sm:text-[13px] {validName(row.name)
												? 'ring-line focus:ring-accent'
												: 'ring-warn focus:ring-warn'}"
											placeholder="FIELD NAME"
											value={row.name}
											spellcheck="false"
											autocapitalize="characters"
											autocomplete="off"
											oninput={(e) => (row.name = e.currentTarget.value.toUpperCase())}
											onblur={() => {
												renaming = null;
												if (!row.name.trim() && row.original === undefined) editor.remove(row);
											}}
											onkeydown={(e) => e.key === 'Enter' && e.currentTarget.blur()}
											{@attach focus}
										/>
									{:else}
										<button
											type="button"
											class="min-w-0 truncate text-left text-[13px] font-semibold {row.removed ? 'text-faint line-through' : 'text-muted'}"
											title={row.name}
											ondblclick={() => !row.removed && (renaming = row.id)}
										>
											{labelOf(row)}
										</button>
									{/if}
								</div>
								<div class="flex min-w-0 flex-1 items-start gap-1">
									{#if long}
										<textarea
											class="min-h-9 w-full min-w-0 resize-none rounded-lg bg-transparent px-2 py-1.5 text-base outline-none placeholder:text-faint placeholder:italic focus:bg-bg focus:ring-2 focus:ring-accent disabled:opacity-50 sm:text-sm"
											rows="1"
											value={value ?? ''}
											placeholder={value === null ? '<multiple values>' : ''}
											disabled={row.removed || editor.saving}
											oninput={(e) => editor.setAll(row, e.currentTarget.value)}
											{@attach grow}
										></textarea>
									{:else}
										<input
											class="h-9 w-full min-w-0 rounded-lg bg-transparent px-2 text-base outline-none placeholder:text-faint placeholder:italic focus:bg-bg focus:ring-2 focus:ring-accent disabled:opacity-50 sm:text-sm"
											value={value ?? ''}
											placeholder={value === null ? '<multiple values>' : ''}
											disabled={row.removed || editor.saving}
											spellcheck="false"
											autocomplete="off"
											oninput={(e) => editor.setAll(row, e.currentTarget.value)}
										/>
									{/if}
									<Menu items={() => rowItems(row)} header={{ title: labelOf(row) }} label="Field actions" />
								</div>
							</li>
						{/each}
					</ul>

					<button
						type="button"
						class="press mt-3 inline-flex h-10 items-center gap-2 rounded-full bg-surface px-4 text-[13px] font-semibold"
						disabled={editor.saving}
						onclick={addField}
					>
						<Plus class="size-4" /> Add field
					</button>
					{#if editor.problem}
						<p class="mt-2 text-xs text-warn">{editor.problem}</p>
					{/if}
				{:else if tab === 'details' && count}
					{@const one = count === 1 ? editor.files[0] : null}
					<dl class="divide-y divide-line rounded-2xl bg-surface">
						{#if one?.info}
							{@render detail('File', one.uri)}
							{@render detail('Codec', one.info.codec)}
							{@render detail('Sample rate', one.info.sampleRate ? `${khz(one.info.sampleRate)} kHz` : null)}
							{@render detail('Bits per sample', one.info.bits)}
							{@render detail('Channels', one.info.channels)}
							{@render detail('Bitrate', one.info.bitrate ? `${one.info.bitrate} kbps` : null)}
							{@render detail('Duration', clock(one.info.duration))}
							{@render detail('File size', bytes(one.info.size))}
							{@render detail('Tags', one.info.tagType)}
						{:else}
							{@render detail('Files', count)}
							{@render detail('Codecs', totals.codecs)}
							{@render detail('Total duration', clock(totals.duration))}
							{@render detail('Total size', bytes(totals.size))}
							{@render detail('Tags', totals.tags)}
						{/if}
					</dl>
					{#if !one}
						<ul class="mt-3 divide-y divide-line rounded-2xl bg-surface text-[13px]">
							{#each editor.files as file (file.uri)}
								<li class="flex justify-between gap-3 px-4 py-2">
									<span class="min-w-0 truncate">{fileName(file.uri)}</span>
									<span class="shrink-0 text-muted tabular-nums">
										{file.info?.codec}{file.info?.sampleRate ? ` · ${khz(file.info.sampleRate)} kHz` : ''}{file.info?.bits
											? ` · ${file.info.bits}-bit`
											: ''}
									</span>
								</li>
							{/each}
						</ul>
					{/if}
				{/if}
			{/if}
		</div>
	</div>
{/if}
