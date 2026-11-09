import { ultraCompState, type IUltraCompStateStateful } from 'ultra-light-js';

export interface IConfirmOptions {
    title?: string;
    message: string;
    confirmLabel?: string;
    cancelLabel?: string;
    onDontAskAgain?: () => void;
}

let resolver: ((result: boolean | null) => void) | null = null;
let dontAskAgainCallback: (() => void) | null = null;

export interface IConfirmModalCtx {
    isVisible: IUltraCompStateStateful<boolean>;
    title: IUltraCompStateStateful<string>;
    message: IUltraCompStateStateful<string>;
    confirmLabel: IUltraCompStateStateful<string>;
    cancelLabel: IUltraCompStateStateful<string>;
    hasDontAskAgain: IUltraCompStateStateful<boolean>;
    dontAskAgain: IUltraCompStateStateful<boolean>;
    confirmDialog: (options: IConfirmOptions) => Promise<boolean | null>;
    resolveConfirmDialog: (result: boolean | null) => void;
}

export const CONFIRM_MODAL_CTX: IConfirmModalCtx = ultraCompState({
    isVisible: false,
    title: 'Are you sure?' as string,
    message: '' as string,
    confirmLabel: 'Confirm' as string,
    cancelLabel: 'Cancel' as string,
    hasDontAskAgain: false,
    dontAskAgain: false,

    confirmDialog: (
        comp: IConfirmModalCtx,
        {
            title = 'Are you sure?',
            message,
            confirmLabel = 'Confirm',
            cancelLabel = 'Cancel',
            onDontAskAgain,
        }: IConfirmOptions,
    ): Promise<boolean | null> => {
        dontAskAgainCallback = onDontAskAgain ?? null;
        comp.title.set(title);
        comp.message.set(message);
        comp.confirmLabel.set(confirmLabel);
        comp.cancelLabel.set(cancelLabel);
        comp.hasDontAskAgain.set(onDontAskAgain !== undefined);
        comp.dontAskAgain.set(false);
        comp.isVisible.set(true);
        return new Promise<boolean | null>((resolve) => {
            resolver = resolve;
        });
    },

    resolveConfirmDialog: (comp: IConfirmModalCtx, result: boolean | null) => {
        if (result && comp.dontAskAgain.get()) dontAskAgainCallback?.();
        comp.isVisible.set(false);
        resolver?.(result);
        resolver = null;
        dontAskAgainCallback = null;
    },
});
