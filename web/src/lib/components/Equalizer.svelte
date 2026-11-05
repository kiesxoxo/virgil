<script lang="ts">
	let { playing = true, class: className = '' }: { playing?: boolean; class?: string } = $props();
</script>

<!-- A level meter, standing still: lit while playing, dimmed while paused. Its bars keep their
     heights, because three low ones side by side read as a "···" menu button. -->
<span class="inline-flex h-3.5 w-3.5 items-end gap-[2px] text-accent {className}" class:opacity-40={!playing} aria-label={playing ? 'Now playing' : 'Paused'}>
	<span class="h-[60%] w-[3px] bg-current"></span>
	<span class="h-full w-[3px] bg-current"></span>
	<span class="h-[40%] w-[3px] bg-current"></span>
</span>
