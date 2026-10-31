// How much of the screen the on-screen keyboard covers. iOS doesn't move
// fixed elements out of its way: the phone's search row is lifted above it
// by hand (PlayerBar), and the tab bar is hidden beneath it (+layout).

class Keyboard {
	/** Height in CSS pixels; 0 while the keyboard is down (or there is none). */
	height = $state(0);

	/** Follows the visual viewport. Returns a cleanup. */
	watch() {
		const viewport = window.visualViewport;
		if (!viewport) return () => {};
		const update = () =>
			(this.height = Math.max(0, Math.round(window.innerHeight - viewport.height - viewport.offsetTop)));
		// Safari moves the visual viewport with every scroll of the page, and reading it lays the
		// page out: only bother while a field has focus, the one time a keyboard can be up.
		const scrolled = () => {
			if (this.height > 0 || document.activeElement?.matches('input, textarea, [contenteditable]')) update();
		};
		update();
		viewport.addEventListener('resize', update);
		viewport.addEventListener('scroll', scrolled);
		return () => {
			viewport.removeEventListener('resize', update);
			viewport.removeEventListener('scroll', scrolled);
		};
	}
}

export const keyboard = new Keyboard();
