export function installLeaveGuard(
    target: Pick<Window, 'addEventListener' | 'removeEventListener'>,
    activeCount: () => number,
): () => void {
    const onBeforeUnload = (e: BeforeUnloadEvent) => {
        if (activeCount() <= 0) return;
        e.preventDefault();
        e.returnValue = '';
    };
    target.addEventListener('beforeunload', onBeforeUnload);
    return () => target.removeEventListener('beforeunload', onBeforeUnload);
}
