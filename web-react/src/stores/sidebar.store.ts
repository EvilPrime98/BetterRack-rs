import { create } from 'zustand';

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

interface ISidebarStore {
    isCompact: boolean;
    setIsCompact: (isCompact: boolean) => void;
    /** Overlay visibility, used below the 800px breakpoint. */
    isExpanded: boolean;
    setIsExpanded: (isExpanded: boolean) => void;
    /** Collapsed-to-zero-width state, used at/above the 800px breakpoint. */
    isCollapsed: boolean;
    setIsCollapsed: (isCollapsed: boolean) => void;
    isPinned: boolean;
    setIsPinned: (isPinned: boolean) => void;
    expandedGroups: Record<string, boolean>;
    toggleGroup: (uid: string) => void;
}

export const useSidebarStore = create<ISidebarStore>((set) => ({
    isCompact: readCompact(),
    setIsCompact: (isCompact) => {
        if (typeof window !== 'undefined') {
            window.localStorage.setItem(COMPACT_KEY, String(isCompact));
        }
        set({ isCompact });
    },
    isExpanded: false,
    setIsExpanded: (isExpanded) => set({ isExpanded }),
    isCollapsed: readCollapsed(),
    setIsCollapsed: (isCollapsed) => {
        if (typeof window !== 'undefined') {
            window.localStorage.setItem(COLLAPSED_KEY, String(isCollapsed));
        }
        set({ isCollapsed });
    },
    isPinned: readPinned(),
    setIsPinned: (isPinned) => {
        if (typeof window !== 'undefined') {
            window.localStorage.setItem(PINNED_KEY, String(isPinned));
            window.localStorage.setItem(COLLAPSED_KEY, 'false');
        }
        set({ isPinned, isCollapsed: false, isExpanded: !isPinned });
    },
    expandedGroups: {},
    toggleGroup: (uid) =>
        set((s) => ({
            expandedGroups: { ...s.expandedGroups, [uid]: !s.expandedGroups[uid] },
        })),
}));
