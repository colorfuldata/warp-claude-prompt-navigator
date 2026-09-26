use super::*;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::terminal::model::find::{FindConfig, RegexDFAs};
use serde_json::Value;
use warp_terminal::model::grid::Dimensions;

const RAIL_WIDTH: f32 = 40.;
const TICK_HEIGHT: f32 = 22.;
const RESTING_TICK_WIDTH: f32 = 12.;
const WAVE_DURATION: Duration = Duration::from_millis(180);
const MAX_TRANSCRIPT_BYTES: u64 = 64 * 1024 * 1024;

fn human_prompt(message: &Value) -> Option<String> {
    if message.get("type")?.as_str()? != "user" {
        return None;
    }
    if message
        .get("isMeta")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return None;
    }
    let content = message.get("message")?.get("content")?;
    let text = match content {
        Value::String(text) => text.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter(|part| part.get("type").and_then(Value::as_str) == Some("text"))
            .filter_map(|part| part.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => return None,
    };
    (!text.trim().is_empty()).then_some(text)
}

fn prompt_from_transcript(path: &str, preview: &str, index: usize, count: usize) -> Option<String> {
    let prompts = prompts_from_transcript(path)?;

    let prefix = preview.trim_end_matches("...").trim();
    if prefix.is_empty() {
        return None;
    }
    let matches_preview = |prompt: &String| prompt.trim().starts_with(prefix);
    let aligned_index = prompts
        .len()
        .checked_sub(count)
        .and_then(|start| start.checked_add(index));
    aligned_index
        .and_then(|index| prompts.get(index))
        .filter(|prompt| matches_preview(prompt))
        .or_else(|| prompts.iter().rev().find(|prompt| matches_preview(prompt)))
        .cloned()
}

fn prompts_from_transcript(path: &str) -> Option<Vec<String>> {
    let claude_dir = std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".claude")))?;
    let allowed_dir = claude_dir.join("projects").canonicalize().ok()?;
    let path = Path::new(path).canonicalize().ok()?;
    if path
        .extension()
        .is_none_or(|extension| extension != "jsonl")
        || !path.starts_with(allowed_dir)
    {
        return None;
    }
    let file = File::open(path).ok()?;
    if file.metadata().ok()?.len() > MAX_TRANSCRIPT_BYTES {
        return None;
    }
    Some(BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| serde_json::from_str::<Value>(&line).ok())
        .filter_map(|message| human_prompt(&message))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transcript_parser_keeps_human_text_and_ignores_tool_results() {
        let human = serde_json::json!({
            "type": "user",
            "message": {"content": [{"type": "text", "text": "First line"},
                                    {"type": "text", "text": "Second line"}]}
        });
        let tool_result = serde_json::json!({
            "type": "user",
            "message": {"content": [{"type": "tool_result", "content": "not a prompt"}]}
        });
        let metadata = serde_json::json!({
            "type": "user",
            "isMeta": true,
            "message": {"content": "not a prompt either"}
        });

        assert_eq!(
            human_prompt(&human).as_deref(),
            Some("First line\nSecond line")
        );
        assert_eq!(human_prompt(&tool_result), None);
        assert_eq!(human_prompt(&metadata), None);
    }
}

struct PromptNavigationEntry {
    prompt: String,
    block_index: Option<BlockIndex>,
    output_absolute_row: Option<u64>,
    mouse_state: MouseStateHandle,
}

struct TickWave {
    from: Vec<f32>,
    target: Option<usize>,
    started: Instant,
}

impl Default for TickWave {
    fn default() -> Self {
        Self {
            from: Vec::new(),
            target: None,
            started: Instant::now(),
        }
    }
}

impl TickWave {
    fn target_width(&self, index: usize) -> f32 {
        match self.target.map(|target| target.abs_diff(index)) {
            Some(0) => 28.,
            Some(1) => 22.,
            Some(2) => 16.,
            _ => RESTING_TICK_WIDTH,
        }
    }

    fn width(&self, index: usize, now: Instant) -> f32 {
        let start = self.from.get(index).copied().unwrap_or(RESTING_TICK_WIDTH);
        let progress = (now.saturating_duration_since(self.started).as_secs_f32()
            / WAVE_DURATION.as_secs_f32())
        .clamp(0., 1.);
        let eased = 1. - (1. - progress).powi(3);
        start + (self.target_width(index) - start) * eased
    }

    fn move_to(&mut self, target: Option<usize>, count: usize, now: Instant) -> bool {
        if self.target == target {
            return false;
        }
        self.from = (0..count).map(|index| self.width(index, now)).collect();
        self.target = target;
        self.started = now;
        true
    }
}

#[derive(Default)]
pub(super) struct PromptNavigatorState {
    entries: Vec<PromptNavigationEntry>,
    pinned_preview: Option<usize>,
    wave: Arc<Mutex<TickWave>>,
}

impl TerminalView {
    pub(super) fn sync_cli_prompt_navigator(&mut self, ctx: &mut ViewContext<Self>) {
        let (mut prompts, transcript_path, current_query) = CLIAgentSessionsModel::as_ref(ctx)
            .session(self.view_id)
            .filter(|session| session.agent == CLIAgent::Claude)
            .map(|session| (
                session.session_context.prompt_history.clone(),
                session.session_context.transcript_path.clone(),
                session.session_context.query.clone(),
            ))
            .unwrap_or_default();

        if let Some(path) = transcript_path.as_deref()
            && let Some(transcript_prompts) = prompts_from_transcript(path)
            && transcript_prompts.len() > prompts.len()
        {
            prompts = transcript_prompts;
        }
        if prompts.is_empty() {
            if let Some(query) = current_query.filter(|query| !query.trim().is_empty()) {
                prompts.push(query);
            }
        }

        if prompts.len() < self.prompt_navigator.entries.len() {
            self.prompt_navigator = PromptNavigatorState::default();
        }
        if prompts.len() == self.prompt_navigator.entries.len() {
            return;
        }

        let model = self.model.lock();
        let block = model.block_list().active_block();
        let (block_index, output_absolute_row) = if model.is_alt_screen_active() {
            (None, None)
        } else {
            let grid = block.output_grid().grid_handler();
            (
                Some(model.block_list().active_block_index()),
                Some(grid.cursor_point().row as u64 + grid.num_lines_truncated()),
            )
        };
        for prompt in prompts
            .into_iter()
            .skip(self.prompt_navigator.entries.len())
        {
            self.prompt_navigator.entries.push(PromptNavigationEntry {
                prompt,
                block_index,
                output_absolute_row,
                mouse_state: MouseStateHandle::default(),
            });
        }
        drop(model);
        ctx.notify();
    }

    pub(super) fn jump_to_cli_prompt(&mut self, index: usize, ctx: &mut ViewContext<Self>) {
        let Some(entry) = self.prompt_navigator.entries.get(index) else {
            return;
        };
        let block_index = entry.block_index.or_else(|| {
            let model = self.model.lock();
            (!model.is_alt_screen_active()).then(|| model.block_list().active_block_index())
        });
        let output_absolute_row = entry.output_absolute_row;
        let prompt = entry.prompt.clone();
        let mut found_row = None;
        if let Some(block_index) = block_index {
            let model = self.model.lock();
            if !model.is_alt_screen_active()
                && let Some(block) = model.block_list().block_at(block_index)
            {
                let grid = block.output_grid();
                let approximate_row = output_absolute_row
                    .and_then(|row| row.checked_sub(grid.grid_handler().num_lines_truncated()))
                    .and_then(|row| usize::try_from(row).ok());
                let needle: String = prompt
                    .lines()
                    .find(|line| !line.trim().is_empty())
                    .unwrap_or("")
                    .chars()
                    .take(48)
                    .collect();
                let mut matching_row = None;
                if !needle.is_empty()
                    && let Ok(dfas) = RegexDFAs::new_with_config(
                        &needle,
                        FindConfig {
                            is_regex_enabled: false,
                            is_case_sensitive: true,
                        },
                    )
                {
                    matching_row = grid
                        .find(&dfas)
                        .map(|range| range.start().row)
                        .min_by_key(|row| row.abs_diff(approximate_row.unwrap_or(*row)));
                }
                let row = matching_row.or_else(|| {
                    approximate_row.filter(|row| *row < grid.grid_handler().history_size())
                });
                if let Some(row) = row {
                    let input_mode = *InputModeSettings::as_ref(ctx).input_mode.value();
                    let viewport = self.viewport_state(model.block_list(), input_mode, ctx);
                    found_row = Some(
                        viewport.top_of_block_in_lines(block_index)
                            + block.output_grid_offset()
                            + row.into_lines(),
                    );
                }
            }
        }

        if let Some(row) = found_row {
            self.prompt_navigator.pinned_preview = None;
            self.scroll_to_row_if_not_visible(row, ctx);
        } else {
            let transcript_path = CLIAgentSessionsModel::as_ref(ctx)
                .session(self.view_id)
                .and_then(|session| session.session_context.transcript_path.clone());
            if let Some(path) = transcript_path
                && let Some(full_prompt) = prompt_from_transcript(
                    &path,
                    &prompt,
                    index,
                    self.prompt_navigator.entries.len(),
                )
                && let Some(entry) = self.prompt_navigator.entries.get_mut(index)
            {
                entry.prompt = full_prompt;
            }
            self.prompt_navigator.pinned_preview =
                (self.prompt_navigator.pinned_preview != Some(index)).then_some(index);
            ctx.notify();
        }
    }

    pub(super) fn render_cli_prompt_navigator(&self, stack: &mut Stack, appearance: &Appearance) {
        let entries = &self.prompt_navigator.entries;
        if entries.is_empty() {
            return;
        }

        let pane_width = self.size_info.pane_width_px().as_f32();
        let pane_height = self.size_info.pane_height_px().as_f32();
        if pane_width < 360. || pane_height < 220. {
            return;
        }

        let tick_spacing = TICK_HEIGHT.min((pane_height - 100.) / entries.len() as f32);
        let rail_height = tick_spacing * entries.len() as f32;
        let right_inset = 42.;
        let preview_width = (pane_width - right_inset - RAIL_WIDTH - 24.).clamp(160., 320.);
        let theme = appearance.theme();
        let any_hovered = entries.iter().any(|entry| {
            entry
                .mouse_state
                .lock()
                .is_ok_and(|state| state.is_hovered())
        });
        let mut rail = Stack::new();
        let now = Instant::now();
        let wave = self.prompt_navigator.wave.lock().unwrap();

        for (index, entry) in entries.iter().enumerate() {
            let prompt = entry.prompt.clone();
            let is_current = index + 1 == entries.len();
            let pinned = !any_hovered && self.prompt_navigator.pinned_preview == Some(index);
            let handle = entry.mouse_state.clone();
            let wave_width = wave.width(index, now);
            let wave_state = self.prompt_navigator.wave.clone();
            let tick_count = entries.len();
            let tick = Hoverable::new(handle, move |state| {
                let is_hovered = state.is_hovered();
                let color = if is_hovered {
                    theme.foreground().into()
                } else if is_current {
                    theme.accent().into_solid()
                } else {
                    theme.sub_text_color(theme.background()).into()
                };
                let mark = ConstrainedBox::new(
                    Container::new(Rect::new().finish())
                        .with_background_color(color)
                        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(2.)))
                        .finish(),
                )
                .with_width(wave_width)
                .with_height(if is_hovered || is_current { 3. } else { 2. })
                .finish();
                let mut tick_stack = Stack::new().with_child(
                    Container::new(mark)
                        .with_padding_left((RAIL_WIDTH - wave_width) / 2.)
                        .with_padding_right((RAIL_WIDTH - wave_width) / 2.)
                        .with_padding_top((TICK_HEIGHT - 3.) / 2.)
                        .with_padding_bottom((TICK_HEIGHT - 3.) / 2.)
                        .finish(),
                );

                if is_current {
                    let glow = ConstrainedBox::new(
                        Container::new(Rect::new().finish())
                            .with_background_color(ColorU::new(0, 207, 241, 38))
                            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.)))
                            .finish(),
                    )
                    .with_width((wave_width + 12.).min(RAIL_WIDTH))
                    .with_height(10.)
                    .finish();
                    tick_stack.add_positioned_child(
                        glow,
                        OffsetPositioning::offset_from_parent(
                            vec2f(0., 0.),
                            ParentOffsetBounds::Unbounded,
                            ParentAnchor::Center,
                            ChildAnchor::Center,
                        ),
                    );
                }

                if is_hovered || pinned {
                    let preview = ConstrainedBox::new(
                        Container::new(
                            Text::new(
                                prompt.clone(),
                                appearance.ui_font_family(),
                                appearance.ui_font_size(),
                            )
                            .with_color(theme.foreground().into())
                            .with_selectable(false)
                            .finish(),
                        )
                        .with_uniform_padding(12.)
                        .with_background(theme.surface_2())
                        .with_border(Border::all(1.).with_border_fill(theme.outline()))
                        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.)))
                        .finish(),
                    )
                    .with_width(preview_width)
                    .with_max_height(120.)
                    .finish();
                    tick_stack.add_positioned_child(
                        preview,
                        OffsetPositioning::offset_from_parent(
                            vec2f(-10., 0.),
                            ParentOffsetBounds::Unbounded,
                            ParentAnchor::MiddleLeft,
                            ChildAnchor::MiddleRight,
                        ),
                    );
                }
                tick_stack.finish()
            })
            .with_cursor(Cursor::PointingHand)
            .on_hover(move |mouse_in, ctx, _, _| {
                let mut wave = wave_state.lock().unwrap();
                let target = if mouse_in {
                    Some(index)
                } else if wave.target == Some(index) {
                    None
                } else {
                    return;
                };
                if wave.move_to(target, tick_count, Instant::now()) {
                    ctx.notify();
                    for frame in 1..=9 {
                        ctx.notify_after(Duration::from_millis(frame * 20));
                    }
                }
            })
            .on_click(move |ctx, _, _| {
                ctx.dispatch_typed_action(TerminalAction::JumpToCLIAgentPrompt(index));
            })
            .finish();

            let tick_top = tick_spacing * index as f32;
            rail.add_positioned_child(
                tick,
                OffsetPositioning::offset_from_parent(
                    vec2f(0., tick_top),
                    ParentOffsetBounds::Unbounded,
                    ParentAnchor::TopLeft,
                    ChildAnchor::TopLeft,
                ),
            );
        }

        let rail = ConstrainedBox::new(rail.finish())
            .with_width(RAIL_WIDTH)
            .with_height(rail_height)
            .finish();
        stack.add_positioned_overlay_child(
            rail,
            OffsetPositioning::offset_from_parent(
                vec2f(-right_inset, 0.),
                ParentOffsetBounds::ParentByPosition,
                ParentAnchor::MiddleRight,
                ChildAnchor::MiddleRight,
            ),
        );
    }
}
