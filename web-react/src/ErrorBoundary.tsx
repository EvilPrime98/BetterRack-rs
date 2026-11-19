import { Component, type ReactNode } from 'react';
import { AppLoader } from '@/components/app-loader/app-loader';

export class ErrorBoundary extends Component<
    {
        children: ReactNode;
    },
    {
        error: Error | null;
    }
> {
    state = { error: null as Error | null };

    static getDerivedStateFromError(error: Error) {
        return { error };
    }

    render() {
        if (this.state.error) {
            return (
                <AppLoader
                    message={
                        this.state.error.stack ?? this.state.error.message ?? 'There was an error'
                    }
                />
            );
        }
        return this.props.children;
    }
}
