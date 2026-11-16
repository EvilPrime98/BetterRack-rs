import { wiki, type WikiComic, type WikiPlugin } from 'better-wiki';
import { useCallback, useRef, useState } from 'react';

const clientMap = new Map<string, WikiPlugin>();
clientMap.set(
    'https://imagecomics.fandom.com',
    wiki({ plugin: 'dc-fandom', url: 'https://imagecomics.fandom.com' }),
);
clientMap.set('https://marvel.fandom.com', wiki({ plugin: 'marvel-fandom' }));
clientMap.set('https://dc.fandom.com', wiki({ plugin: 'dc-fandom' }));

const COVER_SIZE = 220;

export const CREDIT_ROLES: { key: keyof WikiComic['credits']; label: string }[] = [
    { key: 'writers', label: 'Writer' },
    { key: 'artists', label: 'Artist' },
    { key: 'inkers', label: 'Inker' },
    { key: 'colorists', label: 'Colorist' },
    { key: 'letterers', label: 'Letterer' },
    { key: 'editors', label: 'Editor' },
    { key: 'executiveEditors', label: 'Executive Editor' },
];

export const APPEARING_GROUPS: { key: keyof WikiComic['appearing']; label: string }[] = [
    { key: 'featuredCharacters', label: 'Featured Characters' },
    { key: 'supportingCharacters', label: 'Supporting Characters' },
    { key: 'antagonists', label: 'Antagonists' },
    { key: 'otherCharacters', label: 'Other Characters' },
    { key: 'locations', label: 'Locations' },
    { key: 'items', label: 'Items' },
    { key: 'concepts', label: 'Concepts' },
];

export function useComicById() {
    const [comic, setComic] = useState<WikiComic | null>(null);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const requestId = useRef(0);

    const fetchComic = useCallback(
        async (pageId: string | undefined, sourceWiki: string | undefined) => {
            const currentRequest = ++requestId.current;

            setLoading(true);
            setError(null);

            const numericId = Number(pageId);

            if (
                !sourceWiki ||
                !clientMap.has(sourceWiki) ||
                !pageId ||
                !Number.isFinite(numericId)
            ) {
                if (requestId.current === currentRequest) {
                    setError('This comic could not be loaded.');
                    setLoading(false);
                }
                return;
            }

            try {
                const client = clientMap.get(sourceWiki) as WikiPlugin;

                const result = await client.getComicById(numericId, {
                    thumbnailSize: COVER_SIZE,
                });

                if (requestId.current !== currentRequest) return;

                if (!result) {
                    setError('This comic could not be found.');
                } else {
                    setComic(result);
                }
            } catch {
                if (requestId.current === currentRequest)
                    setError('This comic could not be loaded.');
            } finally {
                if (requestId.current === currentRequest) setLoading(false);
            }
        },
        [],
    );

    return {
        comic,
        loading,
        error,
        fetchComic,
    };
}
