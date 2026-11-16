import type { ReactNode } from 'react';
import { BrowserRouter } from 'react-router-dom';
import { AppProvider } from '@/context/AppContext';
import { ReadTypesProvider } from '@/context/ReadTypesContext';
import { MoveFileModalProvider } from '@/context/MoveFileModalContext';
import { NewFolderModalProvider } from '@/context/NewFolderModalContext';
import { DownloadDirModalProvider } from '@/context/DownloadDirModalContext';
import { LinkPickerModalProvider } from '@/context/LinkPickerModalContext';

export function Providers({ children }: { children: ReactNode }) {
    return (
        <BrowserRouter>
            <AppProvider>
                <ReadTypesProvider>
                    <MoveFileModalProvider>
                        <NewFolderModalProvider>
                            <DownloadDirModalProvider>
                                <LinkPickerModalProvider>{children}</LinkPickerModalProvider>
                            </DownloadDirModalProvider>
                        </NewFolderModalProvider>
                    </MoveFileModalProvider>
                </ReadTypesProvider>
            </AppProvider>
        </BrowserRouter>
    );
}
