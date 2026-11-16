import { useEffect, useState } from 'react';
import styles from '@/pages/settings.page.module.css';
import { BRButton } from '@/components/br-button/br-button';
import { ExternalLink } from '@/components/external-link/external-link';
import {
    checkServerUpdate,
    describeUpdate,
    getServerLogs,
    getServerVersion,
    type IServerUpdateInfo,
} from '@/services/server-info.service';

const message = (e: unknown) => (e instanceof Error ? e.message : 'Request failed');

type TUpdateState =
    | { status: 'idle' }
    | { status: 'checking' }
    | { status: 'done'; info: IServerUpdateInfo }
    | { status: 'error'; message: string };

export function ServerInfo() {
    const [version, setVersion] = useState('');
    const [update, setUpdate] = useState<TUpdateState>({ status: 'idle' });
    const [logs, setLogs] = useState<string | null>(null);

    useEffect(() => {
        let active = true;
        getServerVersion()
            .then((v) => active && setVersion(`Server version ${v}`))
            .catch((e) => active && setVersion(`Server version unavailable: ${message(e)}`));
        return () => {
            active = false;
        };
    }, []);

    async function onCheckUpdate() {
        setUpdate({ status: 'checking' });
        try {
            setUpdate({ status: 'done', info: await checkServerUpdate() });
        } catch (e) {
            setUpdate({ status: 'error', message: message(e) });
        }
    }

    async function onShowLogs() {
        setLogs('Loading…');
        try {
            const lines = await getServerLogs();
            setLogs(lines.length ? lines.join('\n') : 'No log lines yet.');
        } catch (e) {
            setLogs(message(e));
        }
    }

    return (
        <div className={styles.field}>
            <p className={styles.empty}>{version}</p>

            <p className={styles.empty}>
                {update.status === 'checking' && 'Checking…'}
                {update.status === 'error' && update.message}
                {update.status === 'done' && (
                    <>
                        {describeUpdate(update.info)}
                        {update.info.updateAvailable && update.info.url && (
                            <>
                                {' '}
                                <ExternalLink href={update.info.url}>Release notes</ExternalLink>
                            </>
                        )}
                    </>
                )}
            </p>

            <BRButton text="Check for updates" variant="secondary" onClick={onCheckUpdate} />
            <BRButton text="Show server logs" variant="secondary" onClick={onShowLogs} />

            {logs !== null && (
                <pre
                    style={{
                        maxHeight: 240,
                        overflow: 'auto',
                        fontSize: 12,
                        whiteSpace: 'pre-wrap',
                    }}
                >
                    {logs}
                </pre>
            )}
        </div>
    );
}
