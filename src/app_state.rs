use crate::diff_core::{DiffOptions, DisplayDiff, build_display_diff, should_auto_diff};

pub const STARTUP_STATUS: &str = "Ready. Paste left and right text.";
pub const STATUS_DIFF_PENDING: &str = "Diff pending...";
pub const STATUS_DIFF_RUNNING: &str = "Diff running...";
pub const STATUS_DIFF_UPDATED: &str = "Diff updated.";
pub const STATUS_NO_DIFFERENCES: &str = "No differences.";
pub const STATUS_LARGE_INPUT: &str = "Large input - press Compare to update.";
pub const STATUS_CLEARED: &str = "Ready. Paste left and right text.";

#[derive(Debug, Clone, PartialEq)]
pub struct DiffRequest {
    pub id: u64,
    pub left: String,
    pub right: String,
    pub options: DiffOptions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyOutcome {
    Applied,
    IgnoredStaleRequest,
    IgnoredBecauseDirty,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffResult {
    pub id: u64,
    pub diff: DisplayDiff,
}

#[derive(Debug, Clone)]
pub struct AppState {
    left: String,
    right: String,
    diff: DisplayDiff,
    has_result: bool,
    options: DiffOptions,
    status: String,
    latest_request_id: u64,
    dirty_since_latest_request: bool,
    dirty: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new(DiffOptions::default())
    }
}

impl AppState {
    pub fn new(options: DiffOptions) -> Self {
        Self {
            left: String::new(),
            right: String::new(),
            diff: DisplayDiff::no_changes("", ""),
            has_result: false,
            options,
            status: STARTUP_STATUS.to_string(),
            latest_request_id: 0,
            dirty_since_latest_request: false,
            dirty: false,
        }
    }

    pub fn left(&self) -> &str {
        &self.left
    }

    pub fn right(&self) -> &str {
        &self.right
    }

    pub fn diff(&self) -> &DisplayDiff {
        &self.diff
    }

    pub fn options(&self) -> &DiffOptions {
        &self.options
    }

    pub fn status(&self) -> &str {
        &self.status
    }

    pub fn should_auto_diff(&self) -> bool {
        should_auto_diff(&self.left, &self.right, &self.options)
    }

    pub fn has_current_diff(&self) -> bool {
        self.has_result && !self.dirty
    }

    pub fn has_stale_diff(&self) -> bool {
        self.has_result && self.dirty
    }

    pub fn set_left(&mut self, value: String) -> bool {
        if self.left == value {
            return self.should_auto_diff();
        }

        self.left = value;
        self.mark_dirty_after_edit()
    }

    pub fn set_right(&mut self, value: String) -> bool {
        if self.right == value {
            return self.should_auto_diff();
        }

        self.right = value;
        self.mark_dirty_after_edit()
    }

    pub fn swap(&mut self) -> bool {
        std::mem::swap(&mut self.left, &mut self.right);
        self.mark_dirty_after_edit()
    }

    pub fn clear(&mut self) {
        self.left.clear();
        self.right.clear();
        self.diff = DisplayDiff::no_changes("", "");
        self.has_result = false;
        self.latest_request_id = self.latest_request_id.saturating_add(1);
        self.dirty_since_latest_request = false;
        self.dirty = false;
        self.status = STATUS_CLEARED.to_string();
    }

    pub fn create_manual_request(&mut self) -> DiffRequest {
        self.create_request()
    }

    pub fn create_auto_request(&mut self) -> Option<DiffRequest> {
        if self.should_auto_diff() {
            Some(self.create_request())
        } else {
            self.status = STATUS_LARGE_INPUT.to_string();
            None
        }
    }

    pub fn apply_result(&mut self, result: DiffResult) -> ApplyOutcome {
        if result.id != self.latest_request_id {
            return ApplyOutcome::IgnoredStaleRequest;
        }

        if self.dirty_since_latest_request {
            return ApplyOutcome::IgnoredBecauseDirty;
        }

        self.diff = result.diff;
        self.has_result = true;
        self.dirty = false;
        self.status = if self.diff.ops.is_empty() {
            STATUS_NO_DIFFERENCES.to_string()
        } else {
            STATUS_DIFF_UPDATED.to_string()
        };

        ApplyOutcome::Applied
    }

    pub fn set_status(&mut self, status: impl Into<String>) {
        self.status = status.into();
    }

    fn mark_dirty_after_edit(&mut self) -> bool {
        self.dirty = true;
        self.dirty_since_latest_request = true;

        if self.should_auto_diff() {
            self.status = STATUS_DIFF_PENDING.to_string();
            true
        } else {
            self.status = STATUS_LARGE_INPUT.to_string();
            false
        }
    }

    fn create_request(&mut self) -> DiffRequest {
        self.latest_request_id = self.latest_request_id.saturating_add(1);
        self.dirty_since_latest_request = false;
        self.status = STATUS_DIFF_RUNNING.to_string();

        DiffRequest {
            id: self.latest_request_id,
            left: self.left.clone(),
            right: self.right.clone(),
            options: self.options,
        }
    }
}

impl DiffRequest {
    pub fn compute(self) -> DiffResult {
        DiffResult {
            id: self.id,
            diff: build_display_diff(&self.left, &self.right, &self.options),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff_core::{DiffOptions, render_unified_diff};

    #[test]
    fn default_state_matches_startup_contract() {
        let state = AppState::default();

        assert_eq!(state.left(), "");
        assert_eq!(state.right(), "");
        assert!(state.diff().ops.is_empty());
        assert_eq!(state.status(), STARTUP_STATUS);
        assert!(!state.has_current_diff());
    }

    #[test]
    fn editing_marks_diff_pending_and_requests_auto_diff_for_small_input() {
        let mut state = AppState::default();

        let should_schedule = state.set_left("left\n".to_string());

        assert!(should_schedule);
        assert_eq!(state.status(), STATUS_DIFF_PENDING);
        assert!(!state.has_current_diff());
    }

    #[test]
    fn large_edit_skips_auto_diff_and_sets_manual_compare_status() {
        let mut state = AppState::default();

        let should_schedule =
            state.set_left("x".repeat(DiffOptions::default().auto_diff_max_bytes + 1));

        assert!(!should_schedule);
        assert_eq!(state.status(), STATUS_LARGE_INPUT);
    }

    #[test]
    fn manual_compare_bypasses_large_input_guard() {
        let mut state = AppState::default();
        state.set_left("x".repeat(DiffOptions::default().auto_diff_max_bytes + 1));

        let request = state.create_manual_request();

        assert_eq!(
            request.left.len(),
            DiffOptions::default().auto_diff_max_bytes + 1
        );
        assert_eq!(state.status(), STATUS_DIFF_RUNNING);
    }

    #[test]
    fn applying_latest_clean_result_updates_diff_and_status() {
        let mut state = AppState::default();
        state.set_left("left\n".to_string());
        state.set_right("right\n".to_string());
        let result = state.create_manual_request().compute();

        let outcome = state.apply_result(result);

        assert_eq!(outcome, ApplyOutcome::Applied);
        assert!(
            render_unified_diff(state.left(), state.right(), state.options()).contains("-left\n")
        );
        assert!(
            render_unified_diff(state.left(), state.right(), state.options()).contains("+right\n")
        );
        assert_eq!(state.status(), STATUS_DIFF_UPDATED);
        assert!(state.has_current_diff());
    }

    #[test]
    fn equal_result_uses_no_differences_status() {
        let mut state = AppState::default();
        state.set_left("same\n".to_string());
        state.set_right("same\n".to_string());
        let result = state.create_manual_request().compute();

        let outcome = state.apply_result(result);

        assert_eq!(outcome, ApplyOutcome::Applied);
        assert!(state.diff().ops.is_empty());
        assert_eq!(state.status(), STATUS_NO_DIFFERENCES);
    }

    #[test]
    fn large_edit_after_current_diff_marks_existing_diff_stale() {
        let mut state = AppState::default();
        state.set_left("left\n".to_string());
        state.set_right("right\n".to_string());
        let result = state.create_manual_request().compute();
        state.apply_result(result);

        state.set_left("x".repeat(DiffOptions::default().auto_diff_max_bytes + 1));

        assert_eq!(state.status(), STATUS_LARGE_INPUT);
        assert!(state.has_stale_diff());
        assert!(!state.has_current_diff());
    }

    #[test]
    fn stale_result_is_ignored_when_newer_request_exists() {
        let mut state = AppState::default();
        state.set_left("old\n".to_string());
        let stale = state.create_manual_request().compute();
        state.set_left("new\n".to_string());
        let latest = state.create_manual_request().compute();

        assert_eq!(state.apply_result(stale), ApplyOutcome::IgnoredStaleRequest);
        assert_eq!(state.apply_result(latest), ApplyOutcome::Applied);
    }

    #[test]
    fn result_is_ignored_if_user_edits_after_request_started() {
        let mut state = AppState::default();
        state.set_left("before\n".to_string());
        let result = state.create_manual_request().compute();
        state.set_left("after\n".to_string());

        assert_eq!(
            state.apply_result(result),
            ApplyOutcome::IgnoredBecauseDirty
        );
        assert_eq!(state.status(), STATUS_DIFF_PENDING);
    }

    #[test]
    fn clear_invalidates_in_flight_results() {
        let mut state = AppState::default();
        state.set_left("left\n".to_string());
        let result = state.create_manual_request().compute();
        state.clear();

        assert_eq!(
            state.apply_result(result),
            ApplyOutcome::IgnoredStaleRequest
        );
        assert_eq!(state.left(), "");
        assert_eq!(state.right(), "");
        assert_eq!(state.diff().ops.len(), 0);
        assert_eq!(state.status(), STATUS_CLEARED);
    }

    #[test]
    fn swap_exchanges_inputs_and_marks_pending() {
        let mut state = AppState::default();
        state.set_left("left\n".to_string());
        state.set_right("right\n".to_string());

        let should_schedule = state.swap();

        assert!(should_schedule);
        assert_eq!(state.left(), "right\n");
        assert_eq!(state.right(), "left\n");
        assert_eq!(state.status(), STATUS_DIFF_PENDING);
    }

    #[test]
    fn setting_same_text_does_not_mark_request_dirty() {
        let mut state = AppState::default();
        state.set_left("same\n".to_string());
        state.set_right("same\n".to_string());
        let result = state.create_manual_request().compute();

        state.set_left("same\n".to_string());
        state.set_right("same\n".to_string());

        assert_eq!(state.apply_result(result), ApplyOutcome::Applied);
        assert!(state.diff().ops.is_empty());
    }
}
