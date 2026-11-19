import { UltraComponent, UltraLink } from 'ultra-light-js';
import styles from './breadcrumbs.module.css';
import { ChevronDownIcon } from '../../icons/chevron.icon';
import { LIBRARY_CONTEXT } from '../../context/library.context';
import type { ILibraryGroup, ILibraryResponseItem } from '../../library.types';

interface ICrumb {
    uid: string;
    name: string;
}

function buildTrail(uid: string, groups: ILibraryGroup[]): ICrumb[] {
    const group = groups.find((g) => g.uid === uid);
    if (group) return [{ uid: group.uid, name: group.name }];

    const byUid = new Map<string, ILibraryResponseItem>();
    const ownerGroup = new Map<string, ILibraryGroup>();

    groups.forEach((g) =>
        g.entries.forEach((entry) => {
            byUid.set(entry.uid, entry);
            ownerGroup.set(entry.uid, g);
        }),
    );

    const trail: ICrumb[] = [];
    let current = byUid.get(uid);

    while (current) {
        trail.unshift({ uid: current.uid, name: current.name });
        if (!current.parentId) {
            const owner = ownerGroup.get(current.uid);
            if (owner) trail.unshift({ uid: owner.uid, name: owner.name });
            break;
        }
        current = byUid.get(current.parentId);
    }

    return trail;
}

export function Breadcrumbs({ uid }: { uid: string }) {
    function renderTrail($nav: HTMLElement) {
        const trail = buildTrail(uid, LIBRARY_CONTEXT.groups.get());

        $nav.innerHTML = '';

        $nav.appendChild(
            UltraLink({
                href: '/',
                className: [styles.crumb],
                children: ['<span>Library</span>'],
            }),
        );

        trail.forEach((crumb, i) => {
            const isCurrent = i === trail.length - 1;

            $nav.appendChild(
                UltraComponent({
                    component: '<span></span>',
                    className: [styles.separator],
                    attributes: { 'aria-hidden': 'true' },
                    children: [ChevronDownIcon({ size: 12 })],
                }),
            );

            $nav.appendChild(
                isCurrent
                    ? UltraComponent({
                          component: `<span>${crumb.name}</span>`,
                          className: [styles.crumb, styles.current],
                          attributes: { 'aria-current': 'page' },
                      })
                    : UltraLink({
                          href: `/${crumb.uid}`,
                          className: [styles.crumb],
                          children: [`<span>${crumb.name}</span>`],
                      }),
            );
        });
    }

    return UltraComponent({
        component: '<nav></nav>',

        className: [styles.breadcrumbs],

        attributes: { 'aria-label': 'Breadcrumb' },

        onMount: [renderTrail],

        trigger: [
            {
                subscriber: LIBRARY_CONTEXT.groups.subscribe,
                triggerFunction: renderTrail,
            },
        ],
    });
}
