import { ThemeProvider } from './contexts/ThemeContext';
import { SearchBar } from './components/SearchBar';
import { GroupTabs } from './components/GroupTabs';
import { ClipboardList } from './components/ClipboardList';
import { useClipboardListener } from './hooks/useClipboardListener';
import { useDatabase } from './hooks/useDatabase';

function AppContent() {
  useDatabase();
  useClipboardListener();

  return (
    <div className="flex flex-col h-screen bg-white dark:bg-gray-900">
      <header className="flex items-center justify-between px-4 py-3 border-b border-gray-200 dark:border-gray-700">
        <h1 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
          📋 剪贴板管理器
        </h1>
      </header>
      <SearchBar />
      <GroupTabs />
      <ClipboardList />
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
