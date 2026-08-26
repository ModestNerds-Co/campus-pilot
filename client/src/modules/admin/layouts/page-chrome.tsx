//
//  campus-pilot
//  page-chrome.tsx - Per-page title + primary action registered into the app-bar
//
//  Campus Pilot shell rule: the top app bar always carries
//  the page title and its primary action — a page that draws its own title
//  and action below the bar states the title twice and buries the action
//  under the title/description/search box on a phone. Pages call usePageChrome()
//  once to register into AdminLayout's app-bar instead of rendering their own.
//

import React, { createContext, useContext, useEffect, useMemo, useState } from "react";

interface PageChromeValue {
  title: string;
  action: React.ReactNode;
}

interface PageChromeContextValue extends PageChromeValue {
  setChrome: (value: PageChromeValue) => void;
}

const DEFAULT_CHROME: PageChromeValue = { title: "Admin", action: null };

const PageChromeContext = createContext<PageChromeContextValue | null>(null);

export const PageChromeProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [chrome, setChrome] = useState<PageChromeValue>(DEFAULT_CHROME);
  const value = useMemo(() => ({ ...chrome, setChrome }), [chrome]);
  return <PageChromeContext.Provider value={value}>{children}</PageChromeContext.Provider>;
};

export function usePageChromeContext() {
  const ctx = useContext(PageChromeContext);
  if (!ctx) throw new Error("usePageChromeContext must be used within PageChromeProvider");
  return ctx;
}

export function usePageChrome(title: string, action: React.ReactNode = null) {
  const { setChrome } = usePageChromeContext();
  useEffect(() => {
    setChrome({ title, action });
    return () => setChrome(DEFAULT_CHROME);
  }, [title, action, setChrome]);
}
