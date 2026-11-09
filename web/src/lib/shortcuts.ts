export interface IShortcut {
    key: string;
    shift?: boolean;
    ctrl?: boolean;
    alt?: boolean;
    preventDefault?: boolean;
    handler: (e: KeyboardEvent) => void;
}

type TKeyEventLike = Pick<
    KeyboardEvent,
    'key' | 'shiftKey' | 'ctrlKey' | 'altKey' | 'metaKey' | 'target' | 'defaultPrevented'
> & { preventDefault?: () => void };

function isEditable(target: EventTarget | null): boolean {
    const el = target as { tagName?: string; isContentEditable?: boolean } | null;
    if (!el?.tagName) return false;
    return ['INPUT', 'TEXTAREA', 'SELECT'].includes(el.tagName) || !!el.isContentEditable;
}

function matches(shortcut: IShortcut, e: TKeyEventLike): boolean {
    return (
        shortcut.key.toLowerCase() === e.key.toLowerCase() &&
        !!shortcut.shift === e.shiftKey &&
        !!shortcut.ctrl === e.ctrlKey &&
        !!shortcut.alt === e.altKey &&
        !e.metaKey
    );
}

export function createShortcutRegistry() {
    const groups: IShortcut[][] = [];

    return {
        register(shortcuts: IShortcut[]): () => void {
            groups.push(shortcuts);
            return () => {
                const at = groups.indexOf(shortcuts);
                if (at >= 0) groups.splice(at, 1);
            };
        },

        handle(e: TKeyEventLike): boolean {
            if (e.defaultPrevented || isEditable(e.target)) return false;
            for (let g = groups.length - 1; g >= 0; --g) {
                const hit = groups[g].find((s) => matches(s, e));
                if (!hit) continue;
                if (hit.preventDefault !== false) e.preventDefault?.();
                hit.handler(e as KeyboardEvent);
                return true;
            }
            return false;
        },
    };
}

export type TShortcutRegistry = ReturnType<typeof createShortcutRegistry>;
