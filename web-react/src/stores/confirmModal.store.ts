import { create } from 'zustand';

export interface IConfirmOptions {
    title?: string;
    message: string;
    confirmLabel?: string;
    cancelLabel?: string;
    onDontAskAgain?: () => void;
}

let resolver: ((result: boolean | null) => void) | null = null;
let dontAskAgainCallback: (() => void) | null = null;

interface IConfirmModalStore {
    isVisible: boolean;
    title: string;
    message: string;
    confirmLabel: string;
    cancelLabel: string;
    hasDontAskAgain: boolean;
    dontAskAgain: boolean;
    setDontAskAgain: (value: boolean) => void;
    confirmDialog: (options: IConfirmOptions) => Promise<boolean | null>;
    resolveConfirmDialog: (result: boolean | null) => void;
}

export const useConfirmModalStore = create<IConfirmModalStore>((set, get) => ({
    isVisible: false,
    title: 'Are you sure?',
    message: '',
    confirmLabel: 'Confirm',
    cancelLabel: 'Cancel',
    hasDontAskAgain: false,
    dontAskAgain: false,

    setDontAskAgain: (value) => set({ dontAskAgain: value }),

    confirmDialog: ({
        title = 'Are you sure?',
        message,
        confirmLabel = 'Confirm',
        cancelLabel = 'Cancel',
        onDontAskAgain,
    }) => {
        dontAskAgainCallback = onDontAskAgain ?? null;
        set({
            title,
            message,
            confirmLabel,
            cancelLabel,
            hasDontAskAgain: onDontAskAgain !== undefined,
            dontAskAgain: false,
            isVisible: true,
        });
        return new Promise<boolean | null>((resolve) => {
            resolver = resolve;
        });
    },

    resolveConfirmDialog: (result) => {
        if (result && get().dontAskAgain) dontAskAgainCallback?.();
        set({ isVisible: false });
        resolver?.(result);
        resolver = null;
        dontAskAgainCallback = null;
    },
}));
