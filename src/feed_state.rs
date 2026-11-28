//! Shared feed navigation transitions.
//!
//! Callers pass the navigation fields by value and apply the returned
//! `FeedAction` to their own fields.

use crate::app::{PageLanding, StoryType};

/// One HN API page: 30 ranked ids.
pub const API_PAGE_SIZE: u32 = 30;

/// The feed fields read by the navigation transitions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FeedNavigation {
    pub tab: StoryType,
    /// Rank offset of the first visible story (0-based).
    pub display_cursor: usize,
    /// Index into the visible window.
    pub selected: usize,
    pub story_count: usize,
    pub page_size: u32,
    pub loading: bool,
    pub stale: bool,
    pub stories_for: Option<(StoryType, u32)>,
}

/// A described change the caller applies to its own fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeedAction {
    /// Nothing to do.
    None,
    /// Selection moved inside the visible window.
    Move { selected: usize },
    /// One page turn; `display_cursor` is the new first visible rank. A `None`
    /// landing leaves the caller's pending landing untouched.
    TurnPage {
        display_cursor: usize,
        landing: Option<PageLanding>,
    },
    /// Tab switch; `page` is the page to show for `tab`.
    SwitchTab { tab: StoryType, page: u32 },
    /// Viewport changed; refill the grid from `from_rank`.
    Resize {
        from_rank: usize,
        count: usize,
        pages_to_fetch: u32,
    },
}

/// Ranks the grid must hold: `[display_cursor, display_cursor + visibleCells)`.
///
/// Returns `(from_rank, count, pages_to_fetch)`: the first rank to show, how many
/// ranks the window holds, and how many aligned `page_size` API pages intersect
/// that window. Shrinking never drops held pages; the caller keeps its cache.
pub fn fill_grid(
    visible_rows: usize,
    columns: usize,
    page_size: u32,
    display_cursor: usize,
) -> (usize, usize, u32) {
    let count = visible_rows.saturating_mul(columns);
    if count == 0 || page_size == 0 {
        return (display_cursor, 0, 0);
    }
    let page_size = page_size as usize;
    let first_page = display_cursor / page_size;
    let last_page = display_cursor.saturating_add(count - 1) / page_size;
    (display_cursor, count, (last_page - first_page + 1) as u32)
}

/// 1-based page number shown next to the feed: `floor(display_cursor / page_size) + 1`.
pub fn stepper(display_cursor: usize, page_size: u32) -> u32 {
    if page_size == 0 {
        return 1;
    }
    (display_cursor / page_size as usize) as u32 + 1
}

/// `step` is how many ranks one page turn advances: the visible cell count for a
/// cursor-aware grid, `page_size` for a page-numbered feed.
pub fn next_story(nav: FeedNavigation, step: usize) -> FeedAction {
    if nav.story_count == 0 || nav.stale {
        return FeedAction::None;
    }
    if nav.selected + 1 < nav.story_count {
        return FeedAction::Move {
            selected: nav.selected + 1,
        };
    }
    if nav.loading {
        return FeedAction::None;
    }
    turn_page(nav, step, Some(PageLanding::Start))
}

/// `step` as in [`next_story`].
pub fn prev_story(nav: FeedNavigation, step: usize) -> FeedAction {
    if nav.stale {
        return FeedAction::None;
    }
    if nav.selected > 0 {
        return FeedAction::Move {
            selected: nav.selected - 1,
        };
    }
    if nav.loading {
        return FeedAction::None;
    }
    turn_page_back(nav, step, Some(PageLanding::End))
}

/// `step` as in [`next_story`].
pub fn next_page(nav: FeedNavigation, step: usize) -> FeedAction {
    turn_page(nav, step, None)
}

/// `step` as in [`next_story`].
pub fn prev_page(nav: FeedNavigation, step: usize) -> FeedAction {
    turn_page_back(nav, step, None)
}

/// Switch to `tab`, restoring the page that feed was last shown on.
pub fn switch_tab(nav: FeedNavigation, tab: StoryType, remembered_page: Option<u32>) -> FeedAction {
    if nav.tab == tab {
        return FeedAction::None;
    }
    // The caller remembers the displayed feed's position before switching, so a
    // displayed-but-not-yet-remembered page wins over the stored entry.
    let page = match nav.stories_for {
        Some((feed, page)) if feed == tab => page,
        _ => remembered_page.unwrap_or(1),
    };
    FeedAction::SwitchTab { tab, page }
}

/// The grid to fill after a viewport change, from the current cursor.
pub fn resize(nav: FeedNavigation, visible_rows: usize, columns: usize) -> FeedAction {
    let (from_rank, count, pages_to_fetch) =
        fill_grid(visible_rows, columns, nav.page_size, nav.display_cursor);
    FeedAction::Resize {
        from_rank,
        count,
        pages_to_fetch,
    }
}

fn turn_page(nav: FeedNavigation, step: usize, landing: Option<PageLanding>) -> FeedAction {
    if nav.tab == StoryType::Saved {
        return FeedAction::None;
    }
    FeedAction::TurnPage {
        display_cursor: nav.display_cursor.saturating_add(step),
        landing,
    }
}

fn turn_page_back(nav: FeedNavigation, step: usize, landing: Option<PageLanding>) -> FeedAction {
    if nav.tab == StoryType::Saved || nav.display_cursor == 0 {
        return FeedAction::None;
    }
    FeedAction::TurnPage {
        display_cursor: nav.display_cursor.saturating_sub(step),
        landing,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn navigation(
        tab: StoryType,
        page_size: u32,
        display_cursor: usize,
        story_count: usize,
    ) -> FeedNavigation {
        FeedNavigation {
            tab,
            display_cursor,
            selected: 0,
            story_count,
            page_size,
            loading: false,
            stale: false,
            stories_for: None,
        }
    }

    #[test]
    fn fill_grid_eleven_cells_fetches_one_page() {
        assert_eq!(fill_grid(11, 1, API_PAGE_SIZE, 0), (0, 11, 1));
        assert_eq!(stepper(0, API_PAGE_SIZE), 1);
    }

    #[test]
    fn fill_grid_twenty_eight_cells_fetches_one_page() {
        assert_eq!(fill_grid(28, 1, API_PAGE_SIZE, 0), (0, 28, 1));
        assert_eq!(stepper(0, API_PAGE_SIZE), 1);
        // Advancing once at 28 cells lands on rank 28, still inside page 1.
        assert_eq!(fill_grid(28, 1, API_PAGE_SIZE, 28), (28, 28, 2));
        assert_eq!(stepper(28, API_PAGE_SIZE), 1);
        assert_eq!(stepper(30, API_PAGE_SIZE), 2);
    }

    #[test]
    fn fill_grid_thirty_eight_cells_fetches_two_pages() {
        assert_eq!(fill_grid(38, 1, API_PAGE_SIZE, 0), (0, 38, 2));
        assert_eq!(stepper(0, API_PAGE_SIZE), 1);
    }

    #[test]
    fn fill_grid_two_columns_doubles_the_window() {
        assert_eq!(fill_grid(19, 2, API_PAGE_SIZE, 0), (0, 38, 2));
        assert_eq!(fill_grid(14, 2, API_PAGE_SIZE, 0), (0, 28, 1));
    }

    #[test]
    fn fill_grid_resize_growth_keeps_the_cursor_and_adds_pages() {
        let narrow = fill_grid(11, 1, API_PAGE_SIZE, 0);
        let wide = fill_grid(38, 1, API_PAGE_SIZE, 0);
        assert_eq!(narrow, (0, 11, 1));
        assert_eq!(wide, (0, 38, 2));
        assert_eq!(narrow.0, wide.0, "resize keeps the first visible rank");
        // Shrinking back never asks for fewer held pages than it already has.
        assert_eq!(fill_grid(11, 1, API_PAGE_SIZE, 0).2, 1);
    }

    #[test]
    fn fill_grid_rank_38_to_39_is_contiguous() {
        let (from, count, pages) = fill_grid(38, 1, API_PAGE_SIZE, 0);
        assert_eq!((from, count, pages), (0, 38, 2));
        let (next_from, next_count, next_pages) = fill_grid(38, 1, API_PAGE_SIZE, from + count);
        assert_eq!((next_from, next_count, next_pages), (38, 38, 2));
        assert_eq!(from + count, next_from, "no rank is skipped or repeated");
        // Rank 38 lives in page 1 (ranks 30..=59), so the two windows share it.
        assert_eq!(38 / API_PAGE_SIZE as usize, 1);
    }

    #[test]
    fn fill_grid_empty_viewport_fetches_nothing() {
        assert_eq!(fill_grid(0, 1, API_PAGE_SIZE, 0), (0, 0, 0));
        assert_eq!(fill_grid(10, 0, API_PAGE_SIZE, 0), (0, 0, 0));
        assert_eq!(fill_grid(10, 1, 0, 0), (0, 0, 0));
    }

    #[test]
    fn next_story_moves_then_turns_the_page() {
        let mut feed = navigation(StoryType::Best, 20, 0, 3);
        assert_eq!(next_story(feed, 20), FeedAction::Move { selected: 1 });
        feed.selected = 2;
        assert_eq!(
            next_story(feed, 20),
            FeedAction::TurnPage {
                display_cursor: 20,
                landing: Some(PageLanding::Start)
            }
        );
        feed.loading = true;
        assert_eq!(next_story(feed, 20), FeedAction::None);
    }

    #[test]
    fn prev_story_at_the_top_turns_back_to_the_previous_page() {
        let mut feed = navigation(StoryType::Best, 20, 40, 3);
        assert_eq!(
            prev_story(feed, 20),
            FeedAction::TurnPage {
                display_cursor: 20,
                landing: Some(PageLanding::End)
            }
        );
        feed.display_cursor = 0;
        assert_eq!(prev_story(feed, 20), FeedAction::None);
        feed.display_cursor = 40;
        feed.selected = 1;
        assert_eq!(prev_story(feed, 20), FeedAction::Move { selected: 0 });
    }

    #[test]
    fn saved_has_no_pages() {
        let feed = navigation(StoryType::Saved, 20, 0, 3);
        assert_eq!(next_page(feed, 20), FeedAction::None);
        assert_eq!(prev_page(feed, 20), FeedAction::None);
    }

    #[test]
    fn switch_tab_prefers_the_displayed_page_over_the_stored_one() {
        let mut feed = navigation(StoryType::New, 20, 0, 3);
        feed.stories_for = Some((StoryType::Top, 3));
        assert_eq!(
            switch_tab(feed, StoryType::Top, Some(5)),
            FeedAction::SwitchTab {
                tab: StoryType::Top,
                page: 3
            }
        );
        feed.stories_for = Some((StoryType::New, 1));
        assert_eq!(
            switch_tab(feed, StoryType::Top, Some(5)),
            FeedAction::SwitchTab {
                tab: StoryType::Top,
                page: 5
            }
        );
        assert_eq!(
            switch_tab(feed, StoryType::Top, None),
            FeedAction::SwitchTab {
                tab: StoryType::Top,
                page: 1
            }
        );
        assert_eq!(switch_tab(feed, StoryType::New, Some(5)), FeedAction::None);
    }

    #[test]
    fn resize_reports_the_grid_to_fill() {
        let feed = navigation(StoryType::Best, API_PAGE_SIZE, 0, 3);
        assert_eq!(
            resize(feed, 38, 1),
            FeedAction::Resize {
                from_rank: 0,
                count: 38,
                pages_to_fetch: 2
            }
        );
    }
}
