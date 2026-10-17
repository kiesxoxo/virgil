export interface Toast {
	id: number;
	message: string;
	kind: 'done' | 'error';
}

let next = 0;

class Toasts {
	items = $state<Toast[]>([]);

	show(message: string, kind: Toast['kind'] = 'error', ms = 3500) {
		const id = next++;
		this.items = [...this.items.filter((t) => t.message !== message), { id, message, kind }];
		setTimeout(() => this.dismiss(id), ms);
	}

	/** A short confirmation, e.g. "Playing next". */
	done(message: string) {
		this.show(message, 'done', 2000);
	}

	dismiss(id: number) {
		this.items = this.items.filter((t) => t.id !== id);
	}
}

export const toasts = new Toasts();
