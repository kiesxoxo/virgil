<script lang="ts">
	let { playing = true, class: className = '' }: { playing?: boolean; class?: string } = $props();
</script>

<!-- A level meter, standing still: tall while playing, low while paused. -->
<span class="inline-flex h-3.5 w-3.5 items-end gap-[2px] text-accent {className}" aria-label={playing ? 'Now playing' : 'Paused'}>
	<span class="w-[3px] bg-current" style:height={playing ? '60%' : '25%'}></span>
	<span class="w-[3px] bg-current" style:height={playing ? '100%' : '25%'}></span>
	<span class="w-[3px] bg-current" style:height={playing ? '40%' : '25%'}></span>
</span>
