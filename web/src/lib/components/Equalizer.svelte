<script lang="ts">
	let { playing = true, class: className = '' }: { playing?: boolean; class?: string } = $props();
</script>

<span class="eq inline-flex h-3.5 w-3.5 items-end gap-[2px] text-accent {className}" class:paused={!playing} aria-label="Now playing">
	<span></span><span></span><span></span>
</span>

<style>
	.eq span {
		width: 3px;
		border-radius: 1px;
		background: currentColor;
		animation: bounce 900ms ease-in-out infinite;
	}
	.eq span:nth-child(1) {
		height: 60%;
		animation-delay: -300ms;
	}
	.eq span:nth-child(2) {
		height: 100%;
		animation-delay: -600ms;
	}
	.eq span:nth-child(3) {
		height: 40%;
	}
	.eq.paused span {
		animation-play-state: paused;
	}
	@keyframes bounce {
		0%,
		100% {
			transform: scaleY(0.35);
		}
		50% {
			transform: scaleY(1);
		}
	}
	.eq span {
		transform-origin: bottom;
	}
	@media (prefers-reduced-motion: reduce) {
		.eq span {
			animation: none;
		}
	}
</style>
