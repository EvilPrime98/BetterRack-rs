import { useEffect, useRef, type SyntheticEvent } from 'react';

export function useModalDialog(isVisible: boolean, onClose: () => void) {
    const ref = useRef<HTMLDialogElement>(null);

    useEffect(() => {
        const dialog = ref.current;
        if (!dialog) return;
        if (isVisible && !dialog.open) dialog.showModal();
        if (!isVisible && dialog.open) dialog.close();
    }, [isVisible]);

    const onCancel = (e: SyntheticEvent<HTMLDialogElement>) => {
        e.preventDefault();
        onClose();
    };

    return { ref, onCancel };
}
