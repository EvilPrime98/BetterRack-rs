import { createContext, useContext } from 'react';

export interface IAppContextValue {
    isLoading: boolean;
    setIsLoading: (isLoading: boolean) => void;
}

export const AppContext = createContext<IAppContextValue | null>(null);

export function useAppContext(): IAppContextValue {
    const ctx = useContext(AppContext);
    if (!ctx) throw new Error('useAppContext must be used within AppProvider');
    return ctx;
}
