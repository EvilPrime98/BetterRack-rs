import { useEffect, useEffectEvent, useLayoutEffect, useRef, useState } from 'react';
import styles from './server-modal.module.css';
import { BRButton } from '@/components/br-button/br-button';
import { BRCheckbox } from '@/components/br-checkbox/br-checkbox';
import { useServerModalStore, getStoredServerUrl } from '@/stores/serverModal.store';
import {
    isAndroidPlatform,
    isRemoteModeEnabled,
    getStoredApiKey,
} from '@/services/server-config.service';

export function ServerModal() {
    const isVisible = useServerModalStore((s) => s.isVisible);
    const isMandatory = useServerModalStore((s) => s.isMandatory);
    const error = useServerModalStore((s) => s.error);

    const [text, setText] = useState('');
    const [remoteMode, setRemoteMode] = useState(false);
    const [apiKey, setApiKey] = useState('');
    const inputRef = useRef<HTMLInputElement>(null);

    const showRemoteModeOption = !isAndroidPlatform();

    const showApiKeyField = !showRemoteModeOption || remoteMode;

    const buildRemoteOpts = () => ({ enabled: showRemoteModeOption && remoteMode, apiKey });

    const cancel = () => useServerModalStore.getState().closeServerModal();
    const submit = () => useServerModalStore.getState().submitServer(text, buildRemoteOpts());

    // Effect Event so the listener always reads the latest text/remoteMode/apiKey
    // without re-registering on every keystroke.
    const onKeydown = useEffectEvent((e: KeyboardEvent) => {
        if (e.key === 'Escape') useServerModalStore.getState().closeServerModal();
        if (e.key === 'Enter') useServerModalStore.getState().submitServer(text, buildRemoteOpts());
    });

    useEffect(() => {
        if (!isVisible) return;
        const handleKeydown = (e: KeyboardEvent) => onKeydown(e);
        document.addEventListener('keydown', handleKeydown);
        return () => document.removeEventListener('keydown', handleKeydown);
    }, [isVisible]);

    // Reseed the fields from storage. Focus the address input again each time the modal becomes visible.
    useLayoutEffect(() => {
        if (!isVisible) return;
        const current = getStoredServerUrl();
        setText(current);
        setRemoteMode(isRemoteModeEnabled());
        setApiKey(getStoredApiKey());
        inputRef.current?.focus();
    }, [isVisible]);

    return (
        <div className={styles.overlay} style={{ display: isVisible ? undefined : 'none' }}>
            <button
                type="button"
                className={styles.backdrop}
                aria-label="Close dialog"
                onClick={cancel}
            />

            <div
                className={styles.modal}
                role="dialog"
                aria-modal="true"
                aria-label="Connect to server"
            >
                <p className={styles.title}>Connect to your server</p>

                <p className={styles.hint}>
                    Enter the address of the BetterRack server on your network.
                </p>

                <input
                    ref={inputRef}
                    className={styles.field}
                    type="text"
                    placeholder="192.168.1.100:3000"
                    aria-label="Server address"
                    value={text}
                    onChange={(e) => setText(e.currentTarget.value)}
                />

                {showRemoteModeOption && (
                    <>
                        <BRCheckbox
                            text="Use this as my library server"
                            checked={remoteMode}
                            onChange={(e) => setRemoteMode(e.currentTarget.checked)}
                        />
                    </>
                )}

                {showApiKeyField && (
                    <input
                        className={styles.field}
                        type="text"
                        placeholder="API key (optional)"
                        aria-label="API key (optional)"
                        value={apiKey}
                        onChange={(e) => setApiKey(e.currentTarget.value)}
                    />
                )}

                <p className={styles.errorText}>{error}</p>

                <div className={styles.actions}>
                    {!isMandatory && (
                        <BRButton text="Cancel" variant="secondary" onClick={cancel} />
                    )}
                    <BRButton text="Connect" variant="classic" onClick={submit} />
                </div>
            </div>
        </div>
    );
}
