import { useState } from 'react';
import { ThemeProvider } from './contexts/ThemeContext';
import { SearchBar } from './components/SearchBar';
import { GroupTabs } from './components/GroupTabs';
import { ClipboardList } from './components/ClipboardList';
import { SettingsPanel } from './components/SettingsPanel';
import { useClipboardListener } from './hooks/useClipboardListener';
import { useDatabase } from './hooks/useDatabase';

function AppContent() {
  const [settingsOpen, setSettingsOpen] = useState(false);
  useDatabase();
  useClipboardListener();

  return (
    <div className="flex flex-col h-screen bg-white dark:bg-gray-900">
      <header className="flex items-center justify-between px-4 py-3 border-b border-gray-200 dark:border-gray-700">
        <h1 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
          📋 剪贴板管理器
        </h1>
        <button
          onClick={() => setSettingsOpen(true)}
          className="p-2 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
          title="设置"
        >
          ⚙️
        </button>
      </header>
      <SearchBar />
      <GroupTabs />
      <ClipboardList />
      <SettingsPanel isOpen={settingsOpen} onClose={() => setSettingsOpen(false)} />
    </div>
  );
}

function App() {
  return (
    <ThemeProvider>
      <AppContent />
    </ThemeProvider>
  );
}

export default App;
