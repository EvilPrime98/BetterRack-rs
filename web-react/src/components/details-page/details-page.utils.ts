import type { ReleaseDate, WikiComic } from 'better-wiki';
import { CREDIT_ROLES, APPEARING_GROUPS } from '@/hooks/useComicById';
import type { CreditRow, AppearingGroup } from './details-page.types';

export function formatReleaseDate(releaseDate?: ReleaseDate | null): string {
    const { releaseMonth, releaseDay, releaseYear } = releaseDate || {};
    if (!releaseMonth || !releaseDay || !releaseYear) return '';
    return [releaseMonth, releaseDay].map((n) => n.padStart(2, '0')).join('/') + `/${releaseYear}`;
}

export function getCreditRows(credits?: WikiComic['credits']): CreditRow[] {
    return CREDIT_ROLES.map(({ key, label }) => ({ label, names: credits?.[key] ?? [] })).filter(
        (row) => row.names.length > 0,
    );
}

export function getAppearingGroups(appearing?: WikiComic['appearing']): AppearingGroup[] {
    return APPEARING_GROUPS.map(({ key, label }) => ({
        label,
        entries: appearing?.[key] ?? [],
    })).filter((group) => group.entries.length > 0);
}
