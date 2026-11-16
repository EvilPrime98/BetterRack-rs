import { createContext, useContext } from 'react';
import type { TReadTypes } from '../library.types';

export function matchesReadFilter(readFilter: TReadTypes, readPer: number): boolean {
    if (readFilter === 'all') return true;
    if (readFilter === 'read') return readPer === 100;
    if (readFilter === 'reading') return readPer > 0 && readPer < 100;
    return readPer === 0;
}

export interface IReadTypesContextValue {
    type: TReadTypes;
    next: () => void;
}

export const ReadTypesContext = createContext<IReadTypesContextValue | null>(null);

export function useReadTypesContext(): IReadTypesContextValue {
    const ctx = useContext(ReadTypesContext);
    if (!ctx) throw new Error('useReadTypesContext must be used within ReadTypesProvider');
    return ctx;
}
