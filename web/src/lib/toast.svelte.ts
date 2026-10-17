export interface Toast {
	id: number;
	message: string;
}

let next = 0;

class Toasts {
	items = $state<Toast[]>([]);

	show(message: string, ms = 3500) {
		const id = next++;
		this.items = [...this.items.filter((t) => t.message !== message), { id, message }];
		setTimeout(() => this.dismiss(id), ms);
	}

	dismiss(id: number) {
		this.items = this.items.filter((t) => t.id !== id);
	}
}

export const toasts = new Toasts();
