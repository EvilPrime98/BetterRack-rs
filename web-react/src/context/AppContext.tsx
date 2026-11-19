import { useMemo, useState, type ReactNode } from 'react';
import { AppContext } from './AppContext.hooks';

export function AppProvider({ children }: { children: ReactNode }) {
    const [isLoading, setIsLoading] = useState(false);
    const value = useMemo(() => ({ isLoading, setIsLoading }), [isLoading]);
    return <AppContext.Provider value={value}>{children}</AppContext.Provider>;
}
