import { useEffect, useState } from 'react';
import type { KeyboardEvent, MouseEvent, ReactNode, RefObject } from 'react';
import styles from './br-context-menu.module.css';

const VIEWPORT_MARGIN = 8;
const MENU_ATTR = 'data-br-context-menu';
const ITEM_SELECTOR = '[role^="menuitem"]';

export type BRMenuItem =
    | { kind: 'action'; label: string; onSelect: () => void; icon?: ReactNode; busy?: boolean }
    | { kind: 'radio'; label: string; checked: boolean; onSelect: () => void; title?: string };

export interface BRMenuGroup {
    id: string;
    items: BRMenuItem[];
    heading?: string;
    scrollable?: boolean;
}

interface BRContextMenuProps {
    groups: BRMenuGroup[];
    'aria-label': string;
    targetRef?: RefObject<HTMLElement | null>;
    className?: string;
}

export function BRContextMenu({ groups, targetRef, className, ...aria }: BRContextMenuProps) {
    const [position, setPosition] = useState<{ x: number; y: number } | null>(null);

    useEffect(() => {
        const target: HTMLElement | Document | null = targetRef ? targetRef.current : document;
        if (!target) return;
        const onContextMenu = (event: Event) => {
            const { clientX, clientY } = event as globalThis.MouseEvent;
            event.preventDefault();
            setPosition({ x: clientX, y: clientY });
        };
        target.addEventListener('contextmenu', onContextMenu);
        return () => target.removeEventListener('contextmenu', onContextMenu);
    }, [targetRef]);

    useEffect(() => {
        if (!position) return;
        const close = () => setPosition(null);
        const closeIfOutside = (event: Event) => {
            if (!(event.target as HTMLElement).closest(`[${MENU_ATTR}]`)) close();
        };
        document.addEventListener('pointerdown', closeIfOutside);
        window.addEventListener('wheel', closeIfOutside, { passive: true });
        window.addEventListener('resize', close);
        window.addEventListener('blur', close);
        return () => {
            document.removeEventListener('pointerdown', closeIfOutside);
            window.removeEventListener('wheel', closeIfOutside);
            window.removeEventListener('resize', close);
            window.removeEventListener('blur', close);
        };
    }, [position]);

    if (!position) return null;

    const close = () => setPosition(null);

    const place = (el: HTMLDivElement | null) => {
        if (!el) return;
        const x = Math.max(
            VIEWPORT_MARGIN,
            Math.min(position.x, window.innerWidth - el.offsetWidth - VIEWPORT_MARGIN),
        );
        const y = Math.max(
            VIEWPORT_MARGIN,
            Math.min(position.y, window.innerHeight - el.offsetHeight - VIEWPORT_MARGIN),
        );
        el.style.left = `${x}px`;
        el.style.top = `${y}px`;
        if (!el.contains(document.activeElement))
            el.querySelector<HTMLElement>(ITEM_SELECTOR)?.focus();
    };

    const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
        if (event.key === 'Escape') {
            event.preventDefault();
            event.stopPropagation();
            return close();
        }
        if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) return;
        event.preventDefault();
        const items = Array.from(event.currentTarget.querySelectorAll<HTMLElement>(ITEM_SELECTOR));
        if (items.length === 0) return;
        if (event.key === 'Home') return items[0].focus();
        if (event.key === 'End') return items[items.length - 1].focus();
        const current = items.indexOf(document.activeElement as HTMLElement);
        const step = event.key === 'ArrowDown' ? 1 : -1;
        items[(current + step + items.length) % items.length].focus();
    };

    const run = (action: () => void) => (event: MouseEvent) => {
        event.stopPropagation();
        action();
        close();
    };

    const renderItem = (item: BRMenuItem) => {
        if (item.kind === 'radio') {
            return (
                <button
                    key={item.label}
                    type="button"
                    role="menuitemradio"
                    aria-checked={item.checked}
                    className={styles.item}
                    title={item.title}
                    onClick={run(item.onSelect)}
                >
                    <span className={styles.text}>{item.label}</span>
                </button>
            );
        }
        return (
            <button
                key={item.label}
                type="button"
                role="menuitem"
                className={[styles.item, styles.action, item.busy ? styles.busy : '']
                    .filter(Boolean)
                    .join(' ')}
                onClick={run(item.onSelect)}
            >
                {item.icon}
                <span className={styles.text}>{item.label}</span>
            </button>
        );
    };

    return (
        <div
            ref={place}
            className={[styles.menu, className].filter(Boolean).join(' ')}
            role="menu"
            {...{ [MENU_ATTR]: '' }}
            {...aria}
            style={{ left: position.x, top: position.y }}
            onKeyDown={onKeyDown}
            onWheel={(event) => event.stopPropagation()}
            onContextMenu={(event) => event.stopPropagation()}
        >
            {groups.map((group) => (
                <div
                    key={group.id}
                    className={styles.group}
                    role="group"
                    aria-label={group.heading}
                >
                    {group.heading && <span className={styles.heading}>{group.heading}</span>}
                    {group.scrollable ? (
                        <div className={styles.scroll}>{group.items.map(renderItem)}</div>
                    ) : (
                        group.items.map(renderItem)
                    )}
                </div>
            ))}
        </div>
    );
}
