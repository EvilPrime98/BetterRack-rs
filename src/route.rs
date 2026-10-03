//! Routing (MIGRATION.md §5.2). GPUI has no router: an `enum Route` plus a back/forward stack.

/// Pages from `react/src/App.tsx`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    /// `/`, `/:uid` (folder/group/series uid). `search` is the `?search=` query that swaps in the
    /// search page.
    Library { uid: Option<String>, search: Option<String> },
    /// `/:uid/reader`
    Reader { uid: String },
    /// `/details/:pageId?sourceWiki=`
    Details { page_id: String, source_wiki: Option<String> },
    Settings,
    Store,
    StoreDownloads,
    /// `/filters?writer=<name>`
    Filtered { writer: String },
    /// `/new`: recently added.
    Recent,
    Reading,
}

impl Route {
    pub fn home() -> Self {
        Self::Library { uid: None, search: None }
    }

    /// Page name for the window title (`"<page> · BetterRack"`, see `documentTitle.store.ts`).
    pub fn title(&self) -> &'static str {
        match self {
            Self::Library { search: Some(_), .. } => "Search",
            Self::Library { .. } => "Library",
            Self::Reader { .. } => "Reader",
            Self::Details { .. } => "Details",
            Self::Settings => "Settings",
            Self::Store => "Store",
            Self::StoreDownloads => "Downloads",
            Self::Filtered { .. } => "Filtered",
            Self::Recent => "Recently added",
            Self::Reading => "Reading",
        }
    }

    /// Reader (and the app loader) are full-bleed: no Header/SideBar `Layout`.
    pub fn uses_layout(&self) -> bool {
        !matches!(self, Self::Reader { .. })
    }
}

pub fn window_title(route: &Route) -> String {
    format!("{} · BetterRack", route.title())
}

/// Back/forward stack. Always holds at least one route.
#[derive(Debug, Clone)]
pub struct History {
    stack: Vec<Route>,
    index: usize,
}

impl Default for History {
    fn default() -> Self {
        Self { stack: vec![Route::home()], index: 0 }
    }
}

impl History {
    pub fn current(&self) -> &Route {
        &self.stack[self.index]
    }

    /// Navigate to `route`, dropping any forward entries. No-op when already there.
    pub fn push(&mut self, route: Route) {
        if *self.current() == route {
            return;
        }
        self.stack.truncate(self.index + 1);
        self.stack.push(route);
        self.index += 1;
    }

    pub fn can_back(&self) -> bool {
        self.index > 0
    }

    pub fn can_forward(&self) -> bool {
        self.index + 1 < self.stack.len()
    }

    pub fn back(&mut self) -> bool {
        if !self.can_back() {
            return false;
        }
        self.index -= 1;
        true
    }

    pub fn forward(&mut self) -> bool {
        if !self.can_forward() {
            return false;
        }
        self.index += 1;
        true
    }

    /// `history.back()` with fallback to `/` (what the React code does).
    pub fn back_or_home(&mut self) {
        if !self.back() {
            self.push(Route::home());
        }
    }
}

/// Emitted by pages and the sidebar; `AppRoot` subscribes and pushes the route.
pub struct Navigate(pub Route);

/// `history.back()` with fallback to `/`.
pub struct GoBack;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_back_forward() {
        let mut h = History::default();
        h.push(Route::Settings);
        h.push(Route::Store);
        assert_eq!(h.current(), &Route::Store);
        assert!(h.back());
        assert_eq!(h.current(), &Route::Settings);
        assert!(h.forward());
        assert_eq!(h.current(), &Route::Store);
    }

    #[test]
    fn push_drops_forward_entries_and_dedupes() {
        let mut h = History::default();
        h.push(Route::Settings);
        h.push(Route::Settings);
        h.back();
        h.push(Route::Reading);
        assert!(!h.can_forward());
        assert_eq!(h.current(), &Route::Reading);
        h.back();
        assert_eq!(h.current(), &Route::home());
    }

    #[test]
    fn back_falls_back_home() {
        let mut h = History::default();
        h.back_or_home();
        assert!(!h.can_back() || h.current() == &Route::home());
        let mut h = History::default();
        h.push(Route::Recent);
        h.back_or_home();
        assert_eq!(h.current(), &Route::home());
    }
}
