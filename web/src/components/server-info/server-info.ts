import { UltraComponent } from 'ultra-light-js';
import { BRButton } from '@/components/br-button/br-button';
import {
    checkServerUpdate,
    describeUpdate,
    getServerLogs,
    getServerVersion,
} from '@/services/server-info.service';
import styles from '@/pages/settings.page.module.css';

export function ServerInfo() {
    let $version: HTMLElement | null = null;
    let $update: HTMLElement | null = null;
    let $logs: HTMLPreElement | null = null;

    const message = (e: unknown) => (e instanceof Error ? e.message : 'Request failed');

    const loadVersion = async () => {
        if (!$version) return;
        try {
            $version.textContent = `Server version ${await getServerVersion()}`;
        } catch (e) {
            $version.textContent = `Server version unavailable: ${message(e)}`;
        }
    };

    const onCheckUpdate = async () => {
        if (!$update) return;
        $update.textContent = 'Checking…';
        try {
            const info = await checkServerUpdate();
            $update.textContent = describeUpdate(info);
            if (info.updateAvailable && info.url) {
                const $link = document.createElement('a');
                $link.href = info.url;
                $link.target = '_blank';
                $link.rel = 'noopener noreferrer';
                $link.textContent = ' Release notes';
                $update.appendChild($link);
            }
        } catch (e) {
            $update.textContent = message(e);
        }
    };

    const onShowLogs = async () => {
        if (!$logs) return;
        $logs.hidden = false;
        $logs.textContent = 'Loading…';
        try {
            const lines = await getServerLogs();
            $logs.textContent = lines.length ? lines.join('\n') : 'No log lines yet.';
            $logs.scrollTop = $logs.scrollHeight;
        } catch (e) {
            $logs.textContent = message(e);
        }
    };

    return UltraComponent({
        component: '<div></div>',
        className: [styles.field],
        children: [
            UltraComponent({
                component: `<p class="${styles.empty}"></p>`,
                onMount: [
                    ($el: HTMLElement) => {
                        $version = $el;
                        void loadVersion();
                    },
                ],
            }),
            UltraComponent({
                component: `<p class="${styles.empty}"></p>`,
                onMount: [($el: HTMLElement) => void ($update = $el)],
            }),
            BRButton({
                text: 'Check for updates',
                variant: 'secondary',
                eventHandler: { click: onCheckUpdate },
            }),
            BRButton({
                text: 'Show server logs',
                variant: 'secondary',
                eventHandler: { click: onShowLogs },
            }),
            UltraComponent({
                component: '<pre hidden></pre>',
                attributes: {
                    style: 'max-height:240px;overflow:auto;font-size:12px;white-space:pre-wrap;',
                },
                onMount: [($el: HTMLElement) => void ($logs = $el as HTMLPreElement)],
            }),
        ],
    });
}
