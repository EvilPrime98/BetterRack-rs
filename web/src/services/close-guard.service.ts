import { CONFIRM_MODAL_CTX } from '@/context/confirm-modal.context';

function describeActiveDownloads(activeDownloads: number): string {
    const noun = activeDownloads === 1 ? 'download is' : 'downloads are';
    return `(${activeDownloads}) ${noun} still in progress. Closing BetterRack will cancel ${activeDownloads === 1 ? 'it' : 'them'}.`;
}

export function registerDesktopCloseGuard(): void {
    window.desktop?.onCloseRequested(async (activeDownloads) => {
        const confirmed = await CONFIRM_MODAL_CTX.confirmDialog({
            title: 'Downloads in progress',
            message: describeActiveDownloads(activeDownloads),
            confirmLabel: 'Close anyway',
            cancelLabel: 'Keep open',
        });

        if (confirmed) window.desktop?.confirmClose();
        else window.desktop?.cancelClose();
    });
}
