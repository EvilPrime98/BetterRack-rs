import { ultraCompState, type IUltraCompStateStateful } from 'ultra-light-js';

const COLLAPSED_KEY = 'better-rack-sidebar-collapsed';

function readCollapsed(): boolean {
    if (typeof window === 'undefined') return false;
    return window.localStorage.getItem(COLLAPSED_KEY) === 'true';
}

const PINNED_KEY = 'better-rack-sidebar-pinned';

function readPinned(): boolean {
    if (typeof window === 'undefined') return false;
    return window.localStorage.getItem(PINNED_KEY) === 'true';
}

const COMPACT_KEY = 'better-rack-sidebar-compact';

function readCompact(): boolean {
    if (typeof window === 'undefined') return false;
    return window.localStorage.getItem(COMPACT_KEY) === 'true';
}

interface ISidebarCtx {
    isCompact: IUltraCompStateStateful<boolean>;
    isExpanded: IUltraCompStateStateful<boolean>;
    isCollapsed: IUltraCompStateStateful<boolean>;
    isPinned: IUltraCompStateStateful<boolean>;
    setIsPinned: (isPinned: boolean) => void;
    expandedGroups: IUltraCompStateStateful<Record<string, boolean>>;
    toggleGroup: (uid: string) => void;
}

export const SIDEBAR_CONTEXT: ISidebarCtx = ultraCompState({
    isCompact: readCompact(),
    isExpanded: false,
    isCollapsed: readCollapsed(),
    isPinned: readPinned(),
    setIsPinned: (comp: ISidebarCtx, isPinned: boolean) => {
        comp.isPinned.set(isPinned);
        comp.isCollapsed.set(false);
        comp.isExpanded.set(!isPinned);
    },
    expandedGroups: {} as Record<string, boolean>,
    toggleGroup: (comp: ISidebarCtx, uid: string) => {
        const current = comp.expandedGroups.get();
        comp.expandedGroups.set({ ...current, [uid]: !current[uid] });
    },
});

SIDEBAR_CONTEXT.isCompact.subscribe(() => {
    if (typeof window === 'undefined') return;
    window.localStorage.setItem(COMPACT_KEY, String(SIDEBAR_CONTEXT.isCompact.get()));
});

SIDEBAR_CONTEXT.isPinned.subscribe(() => {
    if (typeof window === 'undefined') return;
    window.localStorage.setItem(PINNED_KEY, String(SIDEBAR_CONTEXT.isPinned.get()));
});

SIDEBAR_CONTEXT.isCollapsed.subscribe(() => {
    if (typeof window === 'undefined') return;
    window.localStorage.setItem(COLLAPSED_KEY, String(SIDEBAR_CONTEXT.isCollapsed.get()));
});
