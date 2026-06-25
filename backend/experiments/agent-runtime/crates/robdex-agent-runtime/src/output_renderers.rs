use serde::{Deserialize, Serialize};

pub const RENDERER_VERSION: u32 = 1;
pub const DEFAULT_VISIBLE_BYTE_LIMIT: usize = 12_000;
pub const DEFAULT_VISIBLE_LINE_LIMIT: usize = 400;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RenderMetadata {
    pub renderer_name: String,
    pub renderer_version: u32,
    pub raw_byte_count: usize,
    pub raw_line_count: usize,
    pub shown_byte_count: usize,
    pub shown_line_count: usize,
    pub truncated: bool,
    pub omitted: bool,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RenderedOutput {
    pub visible_output: String,
    pub metadata: RenderMetadata,
}

pub fn render_generic(raw: &str, status: &str) -> RenderedOutput {
    let bounded = bounded_visible(raw, DEFAULT_VISIBLE_BYTE_LIMIT, DEFAULT_VISIBLE_LINE_LIMIT);
    let visible = never_worse(raw, &bounded.text).to_string();
    metadata("generic", raw, &visible, bounded.truncated, status)
}

pub fn render_tree_list(raw_plaintext_tree: &str, omitted_entries: usize, status: &str) -> RenderedOutput {
    let bounded = bounded_visible(raw_plaintext_tree, DEFAULT_VISIBLE_BYTE_LIMIT, DEFAULT_VISIBLE_LINE_LIMIT);
    let visible = never_worse(raw_plaintext_tree, &bounded.text).to_string();
    let mut rendered = metadata("tree.list", raw_plaintext_tree, &visible, bounded.truncated || omitted_entries > 0, status);
    rendered.metadata.omitted = omitted_entries > 0 || bounded.truncated;
    rendered
}

fn metadata(renderer_name: &str, raw: &str, shown: &str, truncated: bool, status: &str) -> RenderedOutput {
    RenderedOutput {
        visible_output: shown.to_string(),
        metadata: RenderMetadata {
            renderer_name: renderer_name.to_string(),
            renderer_version: RENDERER_VERSION,
            raw_byte_count: raw.len(),
            raw_line_count: line_count(raw),
            shown_byte_count: shown.len(),
            shown_line_count: line_count(shown),
            truncated,
            omitted: truncated,
            status: status.to_string(),
        },
    }
}

struct Bounded {
    text: String,
    truncated: bool,
}

fn bounded_visible(raw: &str, max_bytes: usize, max_lines: usize) -> Bounded {
    let mut out = String::new();
    let mut truncated = false;
    for (index, line) in raw.lines().enumerate() {
        if index >= max_lines {
            truncated = true;
            break;
        }
        let extra = if out.is_empty() { line.len() } else { line.len() + 1 };
        if out.len() + extra > max_bytes {
            truncated = true;
            break;
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(line);
    }
    if truncated {
        let raw_lines = line_count(raw);
        let shown_lines = line_count(&out);
        let omitted = raw_lines.saturating_sub(shown_lines);
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&format!("... {omitted} entries omitted"));
    }
    Bounded { text: out, truncated }
}

fn never_worse<'a>(raw: &'a str, compact: &'a str) -> &'a str {
    if estimate_tokens(compact) > estimate_tokens(raw) {
        raw
    } else {
        compact
    }
}

fn estimate_tokens(text: &str) -> usize {
    text.len().div_ceil(4)
}

fn line_count(text: &str) -> usize {
    if text.is_empty() { 0 } else { text.lines().count() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_renderer_never_exceeds_raw_token_estimate() {
        let raw = "{}";
        let rendered = render_generic("{}", "completed");
        assert!(estimate_tokens(&rendered.visible_output) <= estimate_tokens(raw));
    }

    #[test]
    fn tree_renderer_bounds_large_output_and_marks_omission() {
        let raw = (0..800).map(|idx| format!("├── file-{idx}.txt")).collect::<Vec<_>>().join("\n");
        let rendered = render_tree_list(&raw, 37, "completed");
        assert!(rendered.visible_output.contains("entries omitted"));
        assert!(rendered.metadata.omitted);
        assert_eq!(rendered.metadata.renderer_name, "tree.list");
    }
}
