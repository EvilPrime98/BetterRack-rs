import { Fragment, useMemo } from 'react';
import { Link } from 'react-router-dom';
import styles from './breadcrumbs.module.css';
import { ChevronDownIcon } from '@/icons/chevron.icon';
import { useLibraryStore } from '@/stores/library.store';
import type { ILibraryGroup, ILibraryResponseItem } from '@/library.types';

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
    const groups = useLibraryStore((s) => s.groups);

    const trail = useMemo(() => buildTrail(uid, groups), [uid, groups]);

    return (
        <nav className={styles.breadcrumbs} aria-label="Breadcrumb">
            <Link to="/" className={styles.crumb}>
                <span>Library</span>
            </Link>

            {trail.map((crumb, i) => {
                const isCurrent = i === trail.length - 1;

                return (
                    <Fragment key={crumb.uid}>
                        <span className={styles.separator} aria-hidden="true">
                            <ChevronDownIcon size={12} />
                        </span>
                        {isCurrent ? (
                            <span
                                className={[styles.crumb, styles.current].filter(Boolean).join(' ')}
                                aria-current="page"
                            >
                                {crumb.name}
                            </span>
                        ) : (
                            <Link to={`/${crumb.uid}`} className={styles.crumb}>
                                <span>{crumb.name}</span>
                            </Link>
                        )}
                    </Fragment>
                );
            })}
        </nav>
    );
}
